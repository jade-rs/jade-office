//! 描画 in the browser build: the Draw tab's pens (colour, thickness,
//! effects), the eraser by stroke or by point, なげなわ選択 with moving and
//! resizing, タッチで描画, インクを図形に変換 and the インク エディター, over the
//! same controller the native app drives (`jade-office-model`'s
//! `word::ink_tools::DrawController`).

use gpui::{
    canvas, div, point, px, rgb, AnyElement, Bounds, ClickEvent, Context, Hsla, PathBuilder, Pixels,
};
use jade_web::word::ink_tools::{
    DrawController, DrawRelease, DrawState, DrawTool, EraserKind, InkGesture, INK_EFFECTS,
};

use super::*;
use crate::ribbon::{dropdown, group, menu_caption, menu_panel, row, Popup};

/// The 描画 tab's lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawPopup {
    Pen(usize),
    Eraser,
    AddPen,
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

fn hex(color: &str) -> u32 {
    u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0)
}

/// A labelled button.
fn button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
    active: bool,
    cx: &mut Context<DocView>,
    act: impl Fn(&mut DocView, &mut Context<DocView>) + 'static,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(palette::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(active, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| act(view, cx)))
        .child(label.into())
}

/// A row of a list.
fn item(
    id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
    checked: bool,
    cx: &mut Context<DocView>,
    act: impl Fn(&mut DocView, &mut Context<DocView>) + 'static,
) -> AnyElement {
    button(id, label, false, cx, move |view, cx| {
        view.close_draw_popup();
        act(view, cx);
    })
    .gap_2()
    .child(div().size_3().flex_none().when(checked, |cell| {
        cell.child(
            gpui::svg()
                .path("icons/check.svg")
                .size_3()
                .text_color(palette::accent()),
        )
    }))
    .into_any_element()
}

/// The 描画 tab's groups.
pub fn ribbon_groups(
    draw: &DrawController,
    open: Option<Popup>,
    cx: &mut Context<DocView>,
) -> Vec<AnyElement> {
    let open = match open {
        Some(Popup::Draw(popup)) => Some(popup),
        _ => None,
    };
    let state = &draw.state;
    let mut tools = vec![
        button(
            "draw-select",
            t("draw.select"),
            state.tool == DrawTool::Select,
            cx,
            |view, cx| view.set_draw_tool(DrawTool::Select, cx),
        )
        .into_any_element(),
        button(
            "draw-lasso",
            t("draw.lasso"),
            state.tool == DrawTool::Lasso,
            cx,
            |view, cx| view.set_draw_tool(DrawTool::Lasso, cx),
        )
        .into_any_element(),
    ];
    let eraser = match state.tool {
        DrawTool::Eraser(kind) => Some(kind),
        _ => None,
    };
    let eraser_menu = (open == Some(DrawPopup::Eraser)).then(|| {
        menu_panel("draw-eraser-menu")
            .w(px(200.0))
            .child(item(
                "draw-eraser-stroke",
                t("draw.eraser.stroke"),
                eraser == Some(EraserKind::Stroke),
                cx,
                |view, cx| view.set_draw_tool(DrawTool::Eraser(EraserKind::Stroke), cx),
            ))
            .child(item(
                "draw-eraser-point",
                t("draw.eraser.point"),
                eraser == Some(EraserKind::Point),
                cx,
                |view, cx| view.set_draw_tool(DrawTool::Eraser(EraserKind::Point), cx),
            ))
            .into_any_element()
    });
    tools.push(dropdown(
        button(
            "draw-eraser",
            t("draw.eraser"),
            eraser.is_some(),
            cx,
            move |view, cx| view.toggle_draw_popup(DrawPopup::Eraser, cx),
        )
        .into_any_element(),
        eraser_menu,
    ));
    for (index, pen) in state.pens.iter().enumerate() {
        let active = state.tool == DrawTool::Pen(index);
        let label = if pen.effect.is_some() {
            t("draw.effect")
        } else {
            t(pen.kind_key())
        };
        let menu = (open == Some(DrawPopup::Pen(index))).then(|| pen_menu(state, index, cx));
        let trigger = div()
            .flex()
            .items_center()
            .child(
                button(("draw-pen", index), label, active, cx, move |view, cx| {
                    view.choose_draw_pen(index, cx)
                })
                .flex_col()
                .child(div().w(px(16.0)).h(px(3.0)).bg(rgb(hex(&pen.color)))),
            )
            .child(
                button(
                    ("draw-pen-list", index),
                    "▾",
                    false,
                    cx,
                    move |view, cx| view.toggle_draw_popup(DrawPopup::Pen(index), cx),
                )
                .px_0p5(),
            )
            .into_any_element();
        tools.push(dropdown(trigger, menu));
    }
    let add_menu = (open == Some(DrawPopup::AddPen)).then(|| {
        menu_panel("draw-add-pen-menu")
            .w(px(180.0))
            .children(
                [
                    (jade_web::word::ink_tools::InkTool::Pen, "draw.pen"),
                    (jade_web::word::ink_tools::InkTool::Pencil, "draw.pencil"),
                    (
                        jade_web::word::ink_tools::InkTool::Highlighter,
                        "draw.highlighter",
                    ),
                ]
                .into_iter()
                .map(|(tool, key)| {
                    item(key, t(key), false, cx, move |view, cx| {
                        let index = view.draw.state.add_pen(tool);
                        view.choose_draw_pen(index, cx);
                    })
                }),
            )
            .into_any_element()
    });
    tools.push(dropdown(
        button("draw-add-pen", "+", false, cx, |view, cx| {
            view.toggle_draw_popup(DrawPopup::AddPen, cx)
        })
        .into_any_element(),
        add_menu,
    ));
    let toggle = |id: &'static str,
                  key: &'static str,
                  on: bool,
                  cx: &mut Context<DocView>,
                  flip: fn(&mut DrawState)| {
        button(id, t(key), on, cx, move |view, cx| {
            flip(&mut view.draw.state);
            cx.notify();
        })
        .into_any_element()
    };
    let mut groups = vec![
        group(t("draw.group.tools"), vec![row(tools)]),
        group(
            t("draw.group.mode"),
            vec![row(vec![toggle(
                "draw-touch",
                "draw.touch",
                state.draw_with_touch,
                cx,
                |state| state.draw_with_touch = !state.draw_with_touch,
            )])],
        ),
        group(
            t("draw.group.convert"),
            vec![row(vec![
                toggle(
                    "draw-ink-to-shape",
                    "draw.ink_to_shape",
                    state.ink_to_shape,
                    cx,
                    |state| state.ink_to_shape = !state.ink_to_shape,
                ),
                toggle(
                    "draw-ink-editor",
                    "draw.ink_editor",
                    state.ink_editor,
                    cx,
                    |state| state.ink_editor = !state.ink_editor,
                ),
            ])],
        ),
    ];
    if draw.selected.is_some() {
        groups.push(group(
            t("draw.group.selection"),
            vec![row(vec![
                button("draw-delete", t("draw.delete"), false, cx, |view, cx| {
                    view.draw_selection_edit(cx, |draw, document| draw.delete_selection(document))
                })
                .into_any_element(),
                button(
                    "draw-to-shapes",
                    t("draw.to_shapes"),
                    false,
                    cx,
                    |view, cx| {
                        view.draw_selection_edit(cx, |draw, document| {
                            draw.convert_selection(document).map(|_| ())
                        })
                    },
                )
                .into_any_element(),
            ])],
        ));
    }
    groups
}

/// A pen's list: 太さ, 色 and 効果.
fn pen_menu(state: &DrawState, index: usize, cx: &mut Context<DocView>) -> AnyElement {
    let pen = &state.pens[index];
    let thicknesses: Vec<AnyElement> = pen
        .thicknesses()
        .iter()
        .copied()
        .enumerate()
        .map(|(slot, mm)| {
            item(
                ("draw-thickness", slot),
                format!("{mm} mm"),
                (pen.thickness_mm - mm).abs() < 1e-6,
                cx,
                move |view, cx| {
                    view.draw.state.set_pen_thickness(index, mm);
                    view.restyle_draw_selection(index, cx);
                },
            )
        })
        .collect();
    let colors: Vec<AnyElement> = pen
        .colors()
        .iter()
        .copied()
        .enumerate()
        .map(|(slot, color)| {
            let chosen = pen.effect.is_none() && pen.color.eq_ignore_ascii_case(color);
            div()
                .id(("draw-color", slot))
                .size(px(16.0))
                .rounded_sm()
                .border_1()
                .border_color(if chosen {
                    palette::accent()
                } else {
                    palette::border()
                })
                .bg(rgb(hex(color)))
                .cursor_pointer()
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.close_draw_popup();
                    view.draw.state.set_pen_color(index, color);
                    view.restyle_draw_selection(index, cx);
                }))
                .into_any_element()
        })
        .collect();
    let effects: Vec<AnyElement> = if pen.tool == jade_web::word::ink_tools::InkTool::Pen {
        INK_EFFECTS
            .iter()
            .copied()
            .enumerate()
            .map(|(slot, (effect, key))| {
                item(
                    ("draw-effect", slot),
                    t(key),
                    pen.effect.as_deref() == Some(effect),
                    cx,
                    move |view, cx| {
                        view.draw.state.set_pen_effect(index, Some(effect));
                        view.restyle_draw_selection(index, cx);
                    },
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    let has_effects = !effects.is_empty();
    menu_panel("draw-pen-menu")
        .w(px(230.0))
        .child(menu_caption(t("draw.thickness")))
        .children(thicknesses)
        .child(menu_caption(t("draw.color")))
        .child(div().flex().flex_wrap().gap_1().p_1().children(colors))
        .children(has_effects.then(|| menu_caption(t("draw.effects"))))
        .children(effects)
        .into_any_element()
}

impl DocView {
    fn close_draw_popup(&mut self) {
        if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.popup = None;
        }
    }

    fn toggle_draw_popup(&mut self, popup: DrawPopup, cx: &mut Context<Self>) {
        if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.toggle(Popup::Draw(popup));
        }
        cx.notify();
    }

    fn set_draw_tool(&mut self, tool: DrawTool, cx: &mut Context<Self>) {
        self.draw.set_tool(tool);
        self.close_draw_popup();
        cx.notify();
    }

    fn choose_draw_pen(&mut self, index: usize, cx: &mut Context<Self>) {
        self.draw.choose_pen(index);
        self.close_draw_popup();
        cx.notify();
    }

    /// An edit through the controller, shown as edits are.
    fn draw_selection_edit(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(
            &mut DrawController,
            &mut jade_web::editing::EditableDocument,
        ) -> Result<(), String>,
    ) {
        let mut draw = std::mem::take(&mut self.draw);
        self.word_edit(cx, |document| edit(&mut draw, document));
        self.draw = draw;
    }

    fn restyle_draw_selection(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.draw.selected.is_none() {
            self.draw.state.tool = DrawTool::Pen(index);
            return cx.notify();
        }
        self.draw_selection_edit(cx, move |draw, document| {
            draw.restyle_selection(document, index).map(|_| ())
        });
    }

    /// A press on page `index` at page px `at` while a drawing tool is in
    /// hand. False when the press is the text's.
    pub(super) fn ink_press(
        &mut self,
        index: usize,
        at: (f64, f64),
        cx: &mut Context<Self>,
    ) -> bool {
        let taken = self.draw.press(index, at);
        if taken {
            cx.notify();
        }
        taken
    }

    /// The pointer moving with the button down, while drawing.
    pub(super) fn ink_drag(
        &mut self,
        position: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.draw.is_drawing() {
            return false;
        }
        let page = self
            .draw
            .gesture
            .as_ref()
            .map(|(page, _)| *page)
            .or_else(|| self.draw.selected.as_ref().map(|(page, _)| *page));
        if let Some(page) = page {
            if let Some((x, y)) = self.page_point(page, position) {
                let top = match &self.document {
                    Document::Pages { document, .. } => document.document_y(page, 0.0),
                    _ => 0.0,
                };
                self.draw.drag_to(page, (x, y - top));
            }
        }
        cx.notify();
        true
    }

    /// The button coming up while drawing.
    pub(super) fn ink_release(&mut self, cx: &mut Context<Self>) {
        if !self.draw.is_drawing() {
            return;
        }
        let mut draw = std::mem::take(&mut self.draw);
        let mut outcome = Ok(DrawRelease::Nothing);
        self.word_edit(cx, |document| {
            outcome = draw.release(document);
            outcome.clone().map(|_| ())
        });
        self.draw = draw;
        if let Ok(DrawRelease::Gesture {
            page,
            gesture,
            points,
        }) = outcome
        {
            if !self.ink_gesture(page, gesture, cx) {
                let draw = std::mem::take(&mut self.draw);
                self.word_edit(cx, |document| draw.ink_instead(document, page, points));
                self.draw = draw;
            }
        }
        cx.notify();
    }

    /// The インク エディター on the text. Whether the gesture applied.
    fn ink_gesture(&mut self, page: usize, gesture: InkGesture, cx: &mut Context<Self>) -> bool {
        let Document::Pages { document, .. } = &mut self.document else {
            return false;
        };
        let top = document.document_y(page, 0.0);
        let at = |document: &jade_web::editing::EditableDocument, (x, y): (f64, f64)| {
            document.position_at(x, top + y)
        };
        let applied = match gesture {
            InkGesture::Delete { from, to } => match (at(document, from), at(document, to)) {
                (Some(start), Some(end)) if start != end => {
                    document.select(start, end).is_ok() && document.delete(true).is_ok()
                }
                _ => false,
            },
            InkGesture::Select((l, t, r, b)) => {
                let middle = (t + b) / 2.0;
                match (at(document, (l, middle)), at(document, (r, middle))) {
                    (Some(start), Some(end)) if start != end => document.select(start, end).is_ok(),
                    _ => false,
                }
            }
            InkGesture::Insert(point) => match at(document, point) {
                Some(caret) => {
                    document.place_caret(caret.0, caret.1);
                    self.draw.state.tool = DrawTool::Select;
                    true
                }
                None => false,
            },
        };
        if applied {
            self.refresh_repainted();
            cx.notify();
        }
        applied
    }

    /// What is being drawn over page `page`.
    pub(super) fn ink_overlay(&self, page: usize, zoom: f32) -> Option<AnyElement> {
        let overlay = self.draw.overlay(page)?;
        let selection = overlay.selection.map(|(l, t, r, b)| {
            div()
                .absolute()
                .left(px(l as f32 * zoom))
                .top(px(t as f32 * zoom))
                .w(px((r - l) as f32 * zoom))
                .h(px((b - t) as f32 * zoom))
                .border_1()
                .border_color(palette::accent())
                .child(
                    div()
                        .absolute()
                        .right(px(-4.0))
                        .bottom(px(-4.0))
                        .size(px(8.0))
                        .bg(palette::accent()),
                )
        });
        let paths = canvas(
            |_, _, _| (),
            move |bounds: Bounds<Pixels>, _, window, _| {
                let place = |(x, y): (f64, f64)| {
                    point(
                        bounds.origin.x + px(x as f32 * zoom),
                        bounds.origin.y + px(y as f32 * zoom),
                    )
                };
                let mut stroke = |points: &[(f64, f64)], width: f32, color: Hsla| {
                    if points.is_empty() {
                        return;
                    }
                    let mut builder = PathBuilder::stroke(px(width.max(1.0)));
                    builder.move_to(place(points[0]));
                    for &next in &points[1..] {
                        builder.line_to(place(next));
                    }
                    if points.len() == 1 {
                        builder.line_to(place((points[0].0 + 0.5, points[0].1)));
                    }
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, color);
                    }
                };
                for moved in &overlay.preview {
                    let color: Hsla = rgb(hex(&moved.trace.brush.color)).into();
                    stroke(
                        &moved.trace.points,
                        moved.trace.brush.width_px as f32 * zoom,
                        color.opacity(0.6),
                    );
                }
                if let Some(line) = overlay.gesture.as_ref() {
                    let mut points = line.points.clone();
                    if line.closed {
                        points.push(points[0]);
                    }
                    let color: Hsla = rgb(hex(&line.color)).into();
                    stroke(
                        &points,
                        line.width as f32 * zoom,
                        color.opacity(line.opacity as f32),
                    );
                }
            },
        )
        .absolute()
        .inset_0()
        .size_full();
        Some(
            div()
                .absolute()
                .inset_0()
                .child(paths)
                .children(selection)
                .into_any_element(),
        )
    }
}

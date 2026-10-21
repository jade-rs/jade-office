//! 描画: the Word view's Draw tab and drawing on the pages — pens, the
//! pencil and the highlighter (colour, thickness, effects), the eraser by
//! stroke or by point, なげなわ選択 with moving and resizing what it caught,
//! タッチで描画, インクを図形に変換 and the インク エディター's gestures. What
//! the strokes do to the document is `jade-office-model`'s
//! `word::ink_tools`; this is the tab, the pointer and the overlay.

use gpui::{
    canvas, div, point, prelude::*, px, rgb, AnyElement, Bounds, ClickEvent, Context, Hsla,
    MouseMoveEvent, PathBuilder, Pixels,
};
use jade_office_model::word::ink_tools::{
    DrawController, DrawRelease, DrawState, DrawTool, EraserKind, InkGesture, INK_EFFECTS,
};

use crate::ribbon::{dropdown, menu_panel, Popup};
use crate::tooltip::Tooltip;
use crate::{i18n, theme, DocumentView};

/// The 描画 tab's lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawPopup {
    /// A pen's thickness, colour and effect.
    Pen(usize),
    /// ストローク消しゴム / ポイント消しゴム.
    Eraser,
    /// ペンの追加.
    AddPen,
}

/// The 描画 tab's state in the view: the shared controller.
pub type DrawSession = DrawController;

fn hex(color: &str) -> u32 {
    u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0)
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The 描画 tab's groups.
pub(crate) fn ribbon_groups(
    view: &DocumentView,
    cx: &mut Context<DocumentView>,
) -> Vec<AnyElement> {
    let draw = &view.draw.state;
    let open = match view.ribbon.popup {
        Some(Popup::Draw(popup)) => Some(popup),
        _ => None,
    };
    let tool_button = |id: &'static str,
                       label: &'static str,
                       active: bool,
                       tool: DrawTool,
                       cx: &mut Context<DocumentView>| {
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .h(px(28.0))
            .px_2()
            .rounded_md()
            .text_sm()
            .text_color(theme::text())
            .cursor_pointer()
            .when(active, |button| button.bg(theme::selected()))
            .hover(|button| button.bg(theme::hovered()))
            .tooltip(Tooltip::text(label))
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                view.set_draw_tool(tool, cx);
            }))
            .child(label)
            .into_any_element()
    };
    let mut tools = vec![
        tool_button(
            "draw-select",
            t("draw.select"),
            draw.tool == DrawTool::Select,
            DrawTool::Select,
            cx,
        ),
        tool_button(
            "draw-lasso",
            t("draw.lasso"),
            draw.tool == DrawTool::Lasso,
            DrawTool::Lasso,
            cx,
        ),
    ];
    // 消しゴム and its kinds.
    let eraser_kind = match draw.tool {
        DrawTool::Eraser(kind) => Some(kind),
        _ => None,
    };
    let eraser_menu = (open == Some(DrawPopup::Eraser)).then(|| {
        menu_panel("draw-eraser-menu")
            .w(px(200.0))
            .children(
                [
                    (EraserKind::Stroke, "draw.eraser.stroke"),
                    (EraserKind::Point, "draw.eraser.point"),
                ]
                .into_iter()
                .map(|(kind, key)| {
                    menu_item(
                        key,
                        t(key),
                        eraser_kind == Some(kind),
                        cx,
                        move |view, cx| view.set_draw_tool(DrawTool::Eraser(kind), cx),
                    )
                }),
            )
            .into_any_element()
    });
    tools.push(dropdown(
        split_button(
            "draw-eraser",
            t("draw.eraser"),
            eraser_kind.is_some(),
            None,
            cx,
            move |view, cx| {
                let kind = eraser_kind.unwrap_or(EraserKind::Stroke);
                view.set_draw_tool(DrawTool::Eraser(kind), cx)
            },
            DrawPopup::Eraser,
        ),
        eraser_menu,
    ));
    // The pens.
    for (index, pen) in draw.pens.iter().enumerate() {
        let active = draw.tool == DrawTool::Pen(index);
        let menu = (open == Some(DrawPopup::Pen(index))).then(|| pen_menu(view, index, cx));
        let label = match pen.effect.as_deref() {
            Some(_) => t("draw.effect"),
            None => t(pen.kind_key()),
        };
        tools.push(dropdown(
            split_button(
                ("draw-pen", index),
                label,
                active,
                Some(hex(&pen.color)),
                cx,
                move |view, cx| view.choose_pen(index, cx),
                DrawPopup::Pen(index),
            ),
            menu,
        ));
    }
    let add_menu = (open == Some(DrawPopup::AddPen)).then(|| {
        menu_panel("draw-add-pen-menu")
            .w(px(180.0))
            .children(
                [
                    (jade_docx_editor::inkml::InkTool::Pen, "draw.pen"),
                    (jade_docx_editor::inkml::InkTool::Pencil, "draw.pencil"),
                    (
                        jade_docx_editor::inkml::InkTool::Highlighter,
                        "draw.highlighter",
                    ),
                ]
                .into_iter()
                .map(|(tool, key)| {
                    menu_item(key, t(key), false, cx, move |view, cx| {
                        let index = view.draw.state.add_pen(tool);
                        view.choose_pen(index, cx);
                    })
                }),
            )
            .into_any_element()
    });
    tools.push(dropdown(
        div()
            .id("draw-add-pen")
            .flex()
            .items_center()
            .h(px(28.0))
            .px_2()
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .hover(|button| button.bg(theme::hovered()))
            .tooltip(Tooltip::text(t("draw.add_pen")))
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                view.ribbon.toggle(Popup::Draw(DrawPopup::AddPen));
                cx.notify();
            }))
            .child("+")
            .into_any_element(),
        add_menu,
    ));

    let toggle = |id: &'static str,
                  key: &'static str,
                  on: bool,
                  cx: &mut Context<DocumentView>,
                  action: fn(&mut DrawState)| {
        div()
            .id(id)
            .flex()
            .items_center()
            .h(px(28.0))
            .px_2()
            .rounded_md()
            .text_sm()
            .cursor_pointer()
            .when(on, |button| button.bg(theme::selected()))
            .hover(|button| button.bg(theme::hovered()))
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                action(&mut view.draw.state);
                cx.notify();
            }))
            .child(t(key))
            .into_any_element()
    };
    let mut groups = vec![
        DocumentView::ribbon_group(t("draw.group.tools"), vec![DocumentView::ribbon_row(tools)])
            .into_any_element(),
        DocumentView::ribbon_group(
            t("draw.group.mode"),
            vec![DocumentView::ribbon_row(vec![toggle(
                "draw-touch",
                "draw.touch",
                draw.draw_with_touch,
                cx,
                |state| state.draw_with_touch = !state.draw_with_touch,
            )])],
        )
        .into_any_element(),
        DocumentView::ribbon_group(
            t("draw.group.convert"),
            vec![DocumentView::ribbon_row(vec![
                toggle(
                    "draw-ink-to-shape",
                    "draw.ink_to_shape",
                    draw.ink_to_shape,
                    cx,
                    |state| state.ink_to_shape = !state.ink_to_shape,
                ),
                toggle(
                    "draw-ink-editor",
                    "draw.ink_editor",
                    draw.ink_editor,
                    cx,
                    |state| state.ink_editor = !state.ink_editor,
                ),
            ])],
        )
        .into_any_element(),
    ];
    // What can be done to the strokes the lasso caught.
    if view.draw.selected.is_some() {
        let action = |id: &'static str,
                      key: &'static str,
                      cx: &mut Context<DocumentView>,
                      act: fn(&mut DocumentView, &mut Context<DocumentView>)| {
            div()
                .id(id)
                .flex()
                .items_center()
                .h(px(28.0))
                .px_2()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| act(view, cx)))
                .child(t(key))
                .into_any_element()
        };
        groups.push(
            DocumentView::ribbon_group(
                t("draw.group.selection"),
                vec![DocumentView::ribbon_row(vec![
                    action("draw-delete", "draw.delete", cx, |view, cx| {
                        view.delete_selected_ink(cx)
                    }),
                    action("draw-to-shapes", "draw.to_shapes", cx, |view, cx| {
                        view.convert_selected_ink(cx)
                    }),
                ])],
            )
            .into_any_element(),
        );
    }
    groups
}

/// A tool button with a chevron that opens its list.
fn split_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    active: bool,
    swatch: Option<u32>,
    cx: &mut Context<DocumentView>,
    press: impl Fn(&mut DocumentView, &mut Context<DocumentView>) + 'static,
    popup: DrawPopup,
) -> AnyElement {
    let id = id.into();
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(28.0))
        .rounded_md()
        .when(active, |button| button.bg(theme::selected()))
        .child(
            div()
                .id(id.clone())
                .flex()
                .flex_col()
                .items_center()
                .px_2()
                .text_sm()
                .text_color(theme::text())
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| press(view, cx)))
                .child(label)
                .children(swatch.map(|color| div().w(px(16.0)).h(px(3.0)).bg(rgb(color)))),
        )
        .child(
            div()
                .id(("draw-chevron", popup_slot(popup)))
                .px_0p5()
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.ribbon.toggle(Popup::Draw(popup));
                    cx.notify();
                }))
                .child(
                    gpui::svg()
                        .path("icons/chevron-down.svg")
                        .size_3()
                        .text_color(theme::text_dim()),
                ),
        )
        .into_any_element()
}

fn popup_slot(popup: DrawPopup) -> usize {
    match popup {
        DrawPopup::Pen(index) => index + 2,
        DrawPopup::Eraser => 0,
        DrawPopup::AddPen => 1,
    }
}

/// A row of a list that runs `act`, closing the list.
fn menu_item(
    id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
    checked: bool,
    cx: &mut Context<DocumentView>,
    act: impl Fn(&mut DocumentView, &mut Context<DocumentView>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .h(px(24.0))
        .pl_1()
        .pr_3()
        .rounded_md()
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|row| row.bg(theme::hovered()))
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
            view.ribbon.popup = None;
            act(view, cx);
        }))
        .child(div().size_4().flex_none().when(checked, |cell| {
            cell.child(
                gpui::svg()
                    .path("icons/check.svg")
                    .size_4()
                    .text_color(theme::accent()),
            )
        }))
        .child(label.into())
        .into_any_element()
}

/// A pen's list: 太さ, 色 and 効果.
fn pen_menu(view: &DocumentView, index: usize, cx: &mut Context<DocumentView>) -> AnyElement {
    let pen = &view.draw.state.pens[index];
    let thicknesses = pen
        .thicknesses()
        .iter()
        .copied()
        .enumerate()
        .map(|(slot, mm)| {
            let checked = (pen.thickness_mm - mm).abs() < 1e-6;
            menu_item(
                ("draw-thickness", slot),
                format!("{mm} mm"),
                checked,
                cx,
                move |view, cx| {
                    view.draw.state.set_pen_thickness(index, mm);
                    view.restyle_selected_ink(index, cx);
                },
            )
        })
        .collect::<Vec<_>>();
    let colors = pen
        .colors()
        .iter()
        .copied()
        .enumerate()
        .map(|(slot, color)| {
            let chosen = pen.effect.is_none() && pen.color.eq_ignore_ascii_case(color);
            div()
                .id(("draw-color", slot))
                .size(px(18.0))
                .rounded_sm()
                .border_1()
                .border_color(if chosen {
                    theme::accent()
                } else {
                    theme::border()
                })
                .bg(rgb(hex(color)))
                .cursor_pointer()
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.ribbon.popup = None;
                    view.draw.state.set_pen_color(index, color);
                    view.restyle_selected_ink(index, cx);
                }))
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let effects = (pen.tool == jade_docx_editor::inkml::InkTool::Pen).then(|| {
        INK_EFFECTS
            .iter()
            .copied()
            .enumerate()
            .map(|(slot, (effect, key))| {
                menu_item(
                    ("draw-effect", slot),
                    t(key),
                    pen.effect.as_deref() == Some(effect),
                    cx,
                    move |view, cx| {
                        view.draw.state.set_pen_effect(index, Some(effect));
                        view.restyle_selected_ink(index, cx);
                    },
                )
            })
            .collect::<Vec<_>>()
    });
    menu_panel("draw-pen-menu")
        .w(px(240.0))
        .child(crate::ribbon::menu_caption(t("draw.thickness")))
        .children(thicknesses)
        .child(crate::ribbon::menu_caption(t("draw.color")))
        .child(div().flex().flex_wrap().gap_1().p_1().children(colors))
        .children(effects.map(|effects| {
            div()
                .flex()
                .flex_col()
                .child(crate::ribbon::menu_caption(t("draw.effects")))
                .children(effects)
        }))
        .into_any_element()
}

impl DocumentView {
    /// Picks up a tool.
    pub(crate) fn set_draw_tool(&mut self, tool: DrawTool, cx: &mut Context<Self>) {
        self.draw.set_tool(tool);
        self.ribbon.popup = None;
        cx.notify();
    }

    /// Picks up a pen, or puts it down.
    pub(crate) fn choose_pen(&mut self, index: usize, cx: &mut Context<Self>) {
        self.draw.choose_pen(index);
        self.ribbon.popup = None;
        cx.notify();
    }

    /// A press on a page while a drawing tool is in hand: a stroke, an
    /// eraser pass, a lasso or a grab on the selection starts. False when the
    /// press is the text's.
    pub(crate) fn ink_press(
        &mut self,
        page: usize,
        at: (f64, f64),
        cx: &mut Context<Self>,
    ) -> bool {
        let taken = self.draw.press(page, at);
        if taken {
            cx.notify();
        }
        taken
    }

    /// The pointer moving with the button down: the stroke grows, or the
    /// selection follows. False when no drawing is under way.
    pub(crate) fn ink_drag(
        &mut self,
        page: usize,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.draw.is_drawing() {
            return false;
        }
        let Some(Some(bounds)) = self.page_bounds.borrow().get(page).copied() else {
            return true;
        };
        let zoom = f64::from(self.zoom);
        let at = (
            f64::from(event.position.x - bounds.origin.x) / zoom,
            f64::from(event.position.y - bounds.origin.y) / zoom,
        );
        self.ink_drag_to(page, at);
        cx.notify();
        true
    }

    /// [`Self::ink_drag`] at a page point.
    pub(crate) fn ink_drag_to(&mut self, page: usize, at: (f64, f64)) {
        self.draw.drag_to(page, at);
    }

    /// The button coming up: the stroke goes into the document, the eraser
    /// takes what it passed over, the lasso catches strokes, the selection
    /// lands where it was dragged, or a gesture acts on the text.
    pub(crate) fn ink_release(&mut self, cx: &mut Context<Self>) {
        if !self.draw.is_drawing() {
            return;
        }
        match self.draw.release(&mut self.document) {
            Ok(DrawRelease::Gesture {
                page,
                gesture,
                points,
            }) => {
                if !self.ink_gesture(page, gesture, cx) {
                    let result = self.draw.ink_instead(&mut self.document, page, points);
                    self.tool_done(result, cx);
                }
            }
            Ok(DrawRelease::Edited) => self.tool_done(Ok(()), cx),
            Ok(DrawRelease::Selected | DrawRelease::Nothing) => cx.notify(),
            Err(error) => self.tool_done(Err(error), cx),
        }
    }

    /// The インク エディター: a gesture over text acts on the text. Whether
    /// it did.
    fn ink_gesture(&mut self, page: usize, gesture: InkGesture, cx: &mut Context<Self>) -> bool {
        let position = |view: &Self, (x, y): (f64, f64)| {
            let document_y = view.document.document_y(page, y);
            view.document.position_at(x, document_y)
        };
        let visible = self.document.page_count().max(1);
        match gesture {
            InkGesture::Delete { from, to } => {
                let (Some(start), Some(end)) = (position(self, from), position(self, to)) else {
                    return false;
                };
                if start == end || self.document.select(start, end, visible).is_err() {
                    return false;
                }
                match self.document.replace_selection("", visible) {
                    Ok(caret) => {
                        self.selection_anchor = None;
                        self.insert_at = caret;
                        self.caret = self.document.place_caret(caret.0, caret.1);
                        self.rebuild_after_edit();
                        cx.notify();
                        true
                    }
                    Err(_) => false,
                }
            }
            InkGesture::Select((l, t, r, b)) => {
                let (Some(start), Some(end)) = (
                    position(self, (l, (t + b) / 2.0)),
                    position(self, (r, (t + b) / 2.0)),
                ) else {
                    return false;
                };
                if start == end || self.document.select(start, end, visible).is_err() {
                    return false;
                }
                self.selection_anchor = Some(start);
                self.insert_at = end;
                self.rebuild_all_images();
                cx.notify();
                true
            }
            InkGesture::Insert(at) => {
                let Some(caret) = position(self, at) else {
                    return false;
                };
                self.insert_at = caret;
                self.caret = self.document.place_caret(caret.0, caret.1);
                self.selection_anchor = None;
                self.draw.state.tool = DrawTool::Select;
                cx.notify();
                true
            }
        }
    }

    pub(crate) fn delete_selected_ink(&mut self, cx: &mut Context<Self>) {
        let result = self.draw.delete_selection(&mut self.document);
        self.tool_done(result, cx);
    }

    pub(crate) fn convert_selected_ink(&mut self, cx: &mut Context<Self>) {
        let result = self.draw.convert_selection(&mut self.document).map(|_| ());
        self.tool_done(result, cx);
    }

    /// A pen's new colour, thickness or effect, given to the selection too.
    fn restyle_selected_ink(&mut self, index: usize, cx: &mut Context<Self>) {
        match self.draw.restyle_selection(&mut self.document, index) {
            Ok(true) => self.tool_done(Ok(()), cx),
            Ok(false) => cx.notify(),
            Err(error) => self.tool_done(Err(error), cx),
        }
    }

    /// What is being drawn on page `page`: the stroke in progress, the
    /// eraser's track, the lasso, and the selection's box and handle.
    pub(crate) fn ink_overlay(&self, page: usize) -> Option<AnyElement> {
        let zoom = self.zoom;
        let overlay = self.draw.overlay(page)?;
        let selection_element = overlay.selection.map(|(l, t, r, b)| {
            div()
                .absolute()
                .left(px(l as f32 * zoom))
                .top(px(t as f32 * zoom))
                .w(px((r - l) as f32 * zoom))
                .h(px((b - t) as f32 * zoom))
                .border_1()
                .border_color(theme::accent())
                .child(
                    div()
                        .absolute()
                        .right(px(-4.0))
                        .bottom(px(-4.0))
                        .size(px(8.0))
                        .bg(theme::accent()),
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
                let stroke =
                    |points: &[(f64, f64)], width: f32, color: Hsla, window: &mut gpui::Window| {
                        if points.is_empty() {
                            return;
                        }
                        let mut builder = PathBuilder::stroke(px(width.max(1.0)));
                        builder.move_to(place(points[0]));
                        for &at in &points[1..] {
                            builder.line_to(place(at));
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
                        window,
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
                        window,
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
                .children(selection_element)
                .into_any_element(),
        )
    }
}

/// The ink session for scenario tests: strokes drawn through the pointer.
#[cfg(test)]
mod tests {
    use super::*;
    use jade_office_model::word::ink_tools::{DrawTool, WordInkTools};

    fn open_contract(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    fn drag(
        view: &mut DocumentView,
        page: usize,
        points: &[(f64, f64)],
        cx: &mut Context<DocumentView>,
    ) {
        assert!(view.ink_press(page, points[0], cx));
        for &at in &points[1..] {
            view.ink_drag_to(page, at);
        }
        view.ink_release(cx);
    }

    fn line(from: (f64, f64), to: (f64, f64), steps: usize) -> Vec<(f64, f64)> {
        (0..=steps)
            .map(|step| {
                let t = step as f64 / steps as f64;
                (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
            })
            .collect()
    }

    /// The pen draws ink on the page, the stroke eraser takes it away and
    /// undo brings it back; with タッチで描画 off, a press is the text's.
    #[gpui::test]
    fn pens_draw_and_the_eraser_erases(cx: &mut gpui::TestAppContext) {
        use crate::Undo;
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            assert!(view.document.ink_strokes().is_empty());
            view.choose_pen(1, cx);
            drag(view, 0, &line((300.0, 600.0), (420.0, 640.0), 12), cx);
            let strokes = view.document.ink_strokes();
            assert_eq!(strokes.len(), 1);
            assert_eq!(strokes[0].page, 0);
            assert_eq!(strokes[0].trace.brush.color, "#E71224");
            assert!((strokes[0].trace.points[0].0 - 300.0).abs() < 0.1);

            // A highlighter stroke, then the stroke eraser through the pen's.
            view.choose_pen(3, cx);
            drag(view, 0, &line((300.0, 700.0), (500.0, 700.0), 10), cx);
            assert_eq!(view.document.ink_strokes().len(), 2);
            view.set_draw_tool(DrawTool::Eraser(EraserKind::Stroke), cx);
            drag(view, 0, &line((360.0, 560.0), (360.0, 660.0), 10), cx);
            let left = view.document.ink_strokes();
            assert_eq!(left.len(), 1);
            assert_eq!(
                left[0].trace.brush.tool,
                jade_docx_editor::inkml::InkTool::Highlighter
            );

            // The point eraser cuts the highlighter's stroke in two.
            view.set_draw_tool(DrawTool::Eraser(EraserKind::Point), cx);
            drag(view, 0, &line((400.0, 680.0), (400.0, 720.0), 6), cx);
            assert_eq!(view.document.ink_strokes().len(), 2);

            // Off, the pen waits and the press goes to the text.
            view.choose_pen(0, cx);
            view.draw.state.draw_with_touch = false;
            assert!(!view.ink_press(0, (100.0, 100.0), cx));
            view.draw.state.draw_with_touch = true;
        });
        cx.dispatch_action(Undo);
        view.update(cx, |view, _| {
            assert_eq!(view.document.ink_strokes().len(), 1, "the cut is undone");
        });
    }

    /// なげなわ選択 catches strokes, a drag moves them and a drag on the
    /// handle resizes them; インクを図形に変換 makes a rectangle of a
    /// rectangle drawn.
    #[gpui::test]
    fn lasso_moves_and_resizes_and_ink_becomes_shapes(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.choose_pen(0, cx);
            drag(view, 0, &line((200.0, 500.0), (260.0, 520.0), 8), cx);
            drag(view, 0, &line((600.0, 500.0), (660.0, 520.0), 8), cx);
            view.set_draw_tool(DrawTool::Lasso, cx);
            drag(
                view,
                0,
                &[
                    (180.0, 480.0),
                    (300.0, 480.0),
                    (300.0, 540.0),
                    (180.0, 540.0),
                ],
                cx,
            );
            let (_, selected) = view.draw.selected.clone().expect("a selection");
            assert_eq!(selected.len(), 1);
            let (l, t, _, _) = view.draw.selection_box().unwrap().1;

            // Move it 50px right and 30px down.
            drag(view, 0, &[(l + 20.0, t + 5.0), (l + 70.0, t + 35.0)], cx);
            let moved = view
                .document
                .ink_strokes()
                .into_iter()
                .find(|stroke| stroke.trace.points[0].0 < 400.0)
                .unwrap();
            assert!(
                (moved.trace.points[0].0 - 250.0).abs() < 0.2,
                "{:?}",
                moved.trace.points[0]
            );
            assert!((moved.trace.points[0].1 - 530.0).abs() < 0.2);
            // Resize from the corner: twice as wide.
            let (l, t, r, b) = view.draw.selection_box().unwrap().1;
            drag(view, 0, &[(r, b), (r + (r - l), b)], cx);
            let (nl, _, nr, _) = view.draw.selection_box().unwrap().1;
            // The points scale with the box; the brush keeps its width.
            assert!(((nr - nl) - 2.0 * (r - l)).abs() < 2.5, "{nl} {nr} {l} {r}");
            let _ = t;
            view.delete_selected_ink(cx);
            assert_eq!(view.document.ink_strokes().len(), 1);

            // A rectangle drawn with インクを図形に変換 on is a shape, not ink.
            view.draw.state.ink_to_shape = true;
            view.choose_pen(0, cx);
            let mut rect = line((100.0, 800.0), (300.0, 800.0), 20);
            rect.extend(line((300.0, 800.0), (300.0, 900.0), 10));
            rect.extend(line((300.0, 900.0), (100.0, 900.0), 20));
            rect.extend(line((100.0, 900.0), (101.0, 802.0), 10));
            let before = view.document.ink_strokes().len();
            drag(view, 0, &rect, cx);
            assert_eq!(view.document.ink_strokes().len(), before);
            let shapes = view
                .document
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
            assert!(shapes >= 1, "the rectangle went in as a shape");
        });
    }

    /// The インク エディター: a line struck through words deletes them, a
    /// caret drawn between them puts the insertion point there; other
    /// strokes stay ink.
    #[gpui::test]
    fn the_ink_editor_acts_on_the_text(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            use jade_office_model::word::WordSession;
            let document = view.document.word_document();
            let paragraph = (0..document.paragraph_count())
                .find(|&index| {
                    document.is_body_paragraph_surface(index)
                        && view
                            .document
                            .paragraph_text(index)
                            .is_some_and(|text| text.chars().count() > 12)
                })
                .expect("a paragraph with text");
            let before = view.document.paragraph_text(paragraph).unwrap();
            let (x, top, height) = view.document.position_metrics((paragraph, 0)).unwrap();
            let (page, y) = view.document.page_and_offset(top);
            let middle = y + height / 2.0;
            view.draw.state.ink_editor = true;
            view.choose_pen(0, cx);
            drag(
                view,
                page,
                &line((x + 1.0, middle), (x + 40.0, middle + 1.0), 10),
                cx,
            );
            let after = view.document.paragraph_text(paragraph).unwrap();
            assert!(after.len() < before.len(), "{before:?} -> {after:?}");
            assert!(
                view.document.ink_strokes().is_empty(),
                "a gesture is not ink"
            );

            // A caret below the line's start: the insertion point goes there.
            let (x, top, height) = view.document.position_metrics((paragraph, 0)).unwrap();
            let (page, y) = view.document.page_and_offset(top);
            let bottom = y + height;
            drag(
                view,
                page,
                &[
                    (x - 4.0, bottom + 2.0),
                    (x, bottom - height * 0.8),
                    (x + 4.0, bottom + 2.0),
                ],
                cx,
            );
            assert_eq!(view.insert_at.0, paragraph);
            assert!(view.document.ink_strokes().is_empty());
        });
    }
}

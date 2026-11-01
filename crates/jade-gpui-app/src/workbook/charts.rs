//! Charts and other drawing objects on the sheet: a click selects one and
//! shows its frame and handles, its border drags it and a handle resizes
//! it, Delete removes it; the toolbar's chart button inserts a chart over
//! the selection, or — with a chart selected — changes its type, legend,
//! title and data, as Excel's Chart Design does.

use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton};
use jade_office_model::charts::{
    dragged, handle_at, ChartAsk, ChartField, ChartKey, ChartKind, Handle, LegendPosition,
    CHART_KINDS, LEGENDS,
};

use super::{caret_line, SheetDrag, Tool, WorkbookView, SHEET_TOOLBAR_HEIGHT};
use crate::{i18n, theme};

/// What the chart button's items do.
#[derive(Clone, Copy)]
pub(super) enum ChartTool {
    /// Insert ▸ Chart over the selection.
    Insert(ChartKind),
    /// Insert ▸ Pictures from a file, at the active cell.
    Picture,
    /// Insert ▸ Shapes, at the active cell: its preset geometry.
    Shape(&'static str),
    /// Chart Design, on the selected chart.
    Kind(ChartKind),
    Legend(Option<LegendPosition>),
    Ask(ChartField),
    Delete,
}

/// The shapes Insert ▸ Shapes offers, with the i18n keys of their names.
const SHAPE_TOOLS: [(&str, &str); 4] = [
    ("rect", "sheet.shape.rectangle"),
    ("ellipse", "sheet.shape.oval"),
    ("roundRect", "sheet.shape.rounded_rectangle"),
    ("rightArrow", "sheet.shape.right_arrow"),
];

/// How far a press has to travel before it drags an object, in px.
const DRAG_SLOP: f64 = 2.0;

impl WorkbookView {
    /// The selected object, when it is a chart.
    pub(super) fn selected_chart(&self) -> Option<u32> {
        self.object.filter(|id| self.book.chart(*id).is_some())
    }

    /// A press at a grid point: on the selected object's handle it starts
    /// a resize, on an object it selects it and starts a move. False when
    /// the press is not an object's, which leaves no object selected.
    pub(super) fn object_press(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let scroll = self.sheet_scroll();
        let handle = self.object.and_then(|id| {
            let rect = self.book.object_rect(id, scroll)?;
            Some((id, handle_at(rect, x, y)?))
        });
        let (id, handle) = match handle {
            Some((id, handle)) => (id, Some(handle)),
            None => match self.book.object_at(x, y, scroll) {
                Some(id) => (id, None),
                None => {
                    if self.object.take().is_some() {
                        cx.notify();
                    }
                    return false;
                }
            },
        };
        let Some(bounds) = self.book.object_bounds(id) else {
            return false;
        };
        self.object = Some(id);
        self.drag = Some(SheetDrag::Object {
            id,
            handle,
            start: (x, y),
            bounds,
            now: bounds,
        });
        cx.notify();
        true
    }

    /// Where an object drag that started at `start` has got to at `(x, y)`.
    pub(super) fn object_drag(
        &self,
        handle: Option<Handle>,
        start: (f64, f64),
        bounds: (f64, f64, f64, f64),
        x: f64,
        y: f64,
    ) -> (f64, f64, f64, f64) {
        dragged(bounds, handle, (x - start.0, y - start.1))
    }

    /// The drag let go: the object lands where it was put.
    pub(super) fn object_drop(
        &mut self,
        id: u32,
        bounds: (f64, f64, f64, f64),
        now: (f64, f64, f64, f64),
        cx: &mut Context<Self>,
    ) {
        let moved = (now.0 - bounds.0).abs() > DRAG_SLOP
            || (now.1 - bounds.1).abs() > DRAG_SLOP
            || (now.2 - bounds.2).abs() > DRAG_SLOP
            || (now.3 - bounds.3).abs() > DRAG_SLOP;
        if !moved {
            cx.notify();
            return;
        }
        let result = self.book.place_object(id, now);
        self.after_write(result, cx);
    }

    /// Keys while an object is selected: Delete and Backspace delete it,
    /// Escape lets it go. Anything else lets it go and goes on to the
    /// cells.
    pub(super) fn object_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.object else {
            return false;
        };
        match key {
            "delete" | "backspace" => {
                self.object = None;
                let result = self.book.delete_object(id);
                self.after_write(result, cx);
                true
            }
            "escape" => {
                self.object = None;
                cx.notify();
                true
            }
            _ => {
                self.object = None;
                cx.notify();
                false
            }
        }
    }

    pub(super) fn chart_tool(&mut self, tool: ChartTool, cx: &mut Context<Self>) {
        let result = match tool {
            ChartTool::Picture => {
                self.pick_picture(cx);
                return;
            }
            ChartTool::Shape(preset) => match self.book.insert_shape(self.active(), preset) {
                Ok(id) => {
                    self.object = Some(id);
                    Ok(true)
                }
                Err(error) => Err(error),
            },
            ChartTool::Insert(kind) => match self.book.insert_chart(self.range(), kind) {
                Ok(id) => {
                    self.object = Some(id);
                    Ok(true)
                }
                Err(error) => Err(error),
            },
            ChartTool::Delete => match self.object.take() {
                Some(id) => self.book.delete_object(id),
                None => Ok(false),
            },
            ChartTool::Kind(kind) => match self.selected_chart() {
                Some(id) => self.book.set_chart(
                    id,
                    &jade_office_model::charts::ChartChange {
                        kind: Some(kind),
                        ..Default::default()
                    },
                ),
                None => Ok(false),
            },
            ChartTool::Legend(legend) => match self.selected_chart() {
                Some(id) => self.book.set_chart(
                    id,
                    &jade_office_model::charts::ChartChange {
                        legend: Some(legend),
                        ..Default::default()
                    },
                ),
                None => Ok(false),
            },
            ChartTool::Ask(field) => {
                if let Some((id, info)) = self
                    .selected_chart()
                    .and_then(|id| Some((id, self.book.chart(id)?)))
                {
                    self.chart_asking = Some(ChartAsk::new(id, field, &info));
                }
                cx.notify();
                return;
            }
        };
        self.after_write(result, cx);
    }

    /// Insert ▸ Pictures: a PNG, JPEG or GIF file, put at the active cell.
    fn pick_picture(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            view.update(cx, |view, cx| match std::fs::read(&path) {
                Ok(bytes) => view.insert_picture(&bytes, cx),
                Err(error) => view.after_write(Err(format!("{}: {error}", path.display())), cx),
            })
            .ok();
        })
        .detach();
    }

    /// `bytes` put on the sheet at the active cell, selected.
    pub(super) fn insert_picture(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let result = match self.book.insert_picture(self.active(), bytes) {
            Ok(id) => {
                self.object = Some(id);
                Ok(true)
            }
            Err(error) => Err(error),
        };
        self.after_write(result, cx);
    }

    /// The chart dialog's OK.
    pub(super) fn take_chart_text(&mut self, cx: &mut Context<Self>) {
        let Some(ask) = self.chart_asking.take() else {
            return;
        };
        let result = self.book.set_chart(ask.id, &ask.change());
        self.after_write(result, cx);
    }

    /// Keys while the chart dialog is open.
    pub(super) fn chart_key(
        &mut self,
        key: &str,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(ask) = self.chart_asking.as_mut() else {
            return false;
        };
        match ask.key(key, command) {
            ChartKey::Take => {
                self.take_chart_text(cx);
                return true;
            }
            ChartKey::Close => self.chart_asking = None,
            ChartKey::Handled => {}
            ChartKey::Unhandled => return false,
        }
        cx.notify();
        true
    }

    /// The chart button's dropdown: Insert ▸ Chart's types, or — with a
    /// chart selected — Chart Design's.
    pub(super) fn chart_menu(&self, cx: &mut Context<Self>) -> gpui::Div {
        let heading = |key: &'static str| {
            div()
                .px_3()
                .pt_2()
                .pb_1()
                .text_color(theme::text_dim())
                .child(i18n::text(key))
        };
        let item = |id: (&'static str, usize),
                    label: &'static str,
                    tool: ChartTool,
                    cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(|item| item.bg(theme::hovered()))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(
                    cx.listener(move |view, _event, _window, cx| view.tool(Tool::Chart(tool), cx)),
                )
        };
        let mut menu = div().w(px(220.0)).py_1();
        if self.selected_chart().is_some() {
            menu = menu.child(heading("chart.change_type"));
            for (index, (kind, label)) in CHART_KINDS.into_iter().enumerate() {
                menu = menu.child(item(
                    ("chart-kind", index),
                    label,
                    ChartTool::Kind(kind),
                    cx,
                ));
            }
            menu = menu.child(heading("chart.legend"));
            for (index, (legend, label)) in LEGENDS.into_iter().enumerate() {
                menu = menu.child(item(
                    ("chart-legend", index),
                    label,
                    ChartTool::Legend(legend),
                    cx,
                ));
            }
            menu = menu
                .child(div().h(px(1.0)).my_1().bg(theme::border()))
                .child(item(
                    ("chart-title", 0),
                    "chart.title_menu",
                    ChartTool::Ask(ChartField::Title),
                    cx,
                ))
                .child(item(
                    ("chart-data", 0),
                    "chart.data_menu",
                    ChartTool::Ask(ChartField::Data),
                    cx,
                ))
                .child(item(
                    ("chart-delete", 0),
                    "chart.delete",
                    ChartTool::Delete,
                    cx,
                ));
        } else {
            menu = menu.child(heading("chart.insert"));
            for (index, (kind, label)) in CHART_KINDS.into_iter().enumerate() {
                menu = menu.child(item(
                    ("chart-insert", index),
                    label,
                    ChartTool::Insert(kind),
                    cx,
                ));
            }
            menu =
                menu.child(div().h(px(1.0)).my_1().bg(theme::border()))
                    .child(item(
                        ("chart-picture", 0),
                        "sheet.insert_picture",
                        ChartTool::Picture,
                        cx,
                    ))
                    .children(SHAPE_TOOLS.into_iter().enumerate().map(
                        |(index, (preset, label))| {
                            item(("chart-shape", index), label, ChartTool::Shape(preset), cx)
                        },
                    ));
        }
        menu
    }

    /// The selected object's frame and handles, over the grid — where a
    /// drag has it, while one is on.
    pub(super) fn object_frame(&self) -> Option<gpui::Div> {
        let id = self.object?;
        let scroll = self.sheet_scroll();
        let mut rect = self.book.object_rect(id, scroll)?;
        if let Some(SheetDrag::Object { bounds, now, .. }) = self.drag {
            rect = (
                rect.0 + now.0 - bounds.0,
                rect.1 + now.1 - bounds.1,
                now.2,
                now.3,
            );
        }
        let zoom = f64::from(self.zoom);
        let (x, y, width, height) = (rect.0 * zoom, rect.1 * zoom, rect.2 * zoom, rect.3 * zoom);
        let handles = Handle::ALL.into_iter().map(|handle| {
            let (fx, fy) = handle.at();
            div()
                .absolute()
                .left(px((width * fx - 4.0) as f32))
                .top(px((height * fy - 4.0) as f32))
                .size(px(8.0))
                .rounded_full()
                .border_1()
                .border_color(theme::accent())
                .bg(gpui::white())
        });
        Some(
            div()
                .absolute()
                .left(px(x as f32))
                .top(px(y as f32))
                .w(px(width as f32))
                .h(px(height as f32))
                .border_1()
                .border_color(theme::accent())
                .children(handles),
        )
    }

    /// Chart Title… and Select Data…: the text, OK and Cancel.
    pub(super) fn chart_dialog(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let ask = self.chart_asking.as_ref()?;
        let button = |id: &'static str,
                      label: &'static str,
                      primary: bool,
                      cx: &mut Context<Self>,
                      take: bool| {
            div()
                .id(id)
                .px_3()
                .py_1()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .cursor_pointer()
                .when(primary, |button| {
                    button.bg(theme::accent()).text_color(gpui::white())
                })
                .hover(|button| button.opacity(0.85))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if take {
                        view.take_chart_text(cx);
                    } else {
                        view.chart_asking = None;
                        cx.notify();
                    }
                }))
        };
        let panel = div()
            .w(px(320.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .shadow_lg()
            .text_xs()
            .text_color(theme::text())
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text_sm().child(i18n::text(ask.field.label_key())))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme::accent())
                    .bg(theme::canvas())
                    .child(caret_line(&ask.text, theme::accent().into(), false)),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("chart-cancel", "cf.cancel", false, cx, false))
                    .child(button("chart-ok", "cf.ok", true, cx, true)),
            );
        Some(
            div()
                .absolute()
                .top(px(SHEET_TOOLBAR_HEIGHT + 60.0))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(panel),
        )
    }
}

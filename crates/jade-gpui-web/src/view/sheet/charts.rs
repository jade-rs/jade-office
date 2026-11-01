//! Charts and other drawing objects on the sheet: a click selects one and
//! shows its frame and handles, its border drags it and a handle resizes
//! it, Delete removes it; the toolbar's chart button inserts a chart over
//! the selection, or — with a chart selected — changes its type, legend,
//! title and data, as Excel's Chart Design does.

use jade_web::charts::{
    dragged, handle_at, ChartAsk, ChartChange, ChartField, ChartKey, ChartKind, Handle,
    LegendPosition, CHART_KINDS, LEGENDS,
};

use super::*;
use jade_web::sheet::SHAPES;

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

/// How far a press has to travel before it drags an object, in px.
const DRAG_SLOP: f64 = 2.0;

impl Sheet {
    /// The selected object, when it is a chart.
    pub(super) fn selected_chart(&self) -> Option<u32> {
        self.object.filter(|id| self.book.chart(*id).is_some())
    }
}

impl DocView {
    /// A press at a grid point: on the selected object's handle it starts
    /// a resize, on an object it selects it and starts a move. False when
    /// the press is not an object's, which leaves no object selected.
    pub(super) fn object_press(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let zoom = self.shown_zoom;
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        let handle = sheet.object.and_then(|id| {
            let rect = sheet.book.object_rect(id, scroll)?;
            Some((id, handle_at(rect, x, y)?))
        });
        let (id, handle) = match handle {
            Some((id, handle)) => (id, Some(handle)),
            None => match sheet.book.object_at(x, y, scroll) {
                Some(id) => (id, None),
                None => {
                    if sheet.object.take().is_some() {
                        cx.notify();
                    }
                    return false;
                }
            },
        };
        let Some(bounds) = sheet.book.object_bounds(id) else {
            return false;
        };
        sheet.object = Some(id);
        sheet.drag = Some(SheetDrag::Object {
            id,
            handle,
            start: (x, y),
            bounds,
            now: bounds,
        });
        cx.notify();
        true
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
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        if !moved {
            cx.notify();
            return;
        }
        let result = sheet.book.place_object(id, now);
        self.edited(result, cx);
    }

    /// Keys while an object is selected: Delete and Backspace delete it,
    /// Escape lets it go. Anything else lets it go and goes on to the
    /// cells.
    pub(super) fn object_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        let Some(id) = sheet.object.take() else {
            return false;
        };
        match key {
            "delete" | "backspace" => {
                let result = sheet.book.delete_object(id);
                self.edited(result, cx);
                true
            }
            "escape" => {
                cx.notify();
                true
            }
            _ => {
                cx.notify();
                false
            }
        }
    }

    pub(super) fn chart_tool(&mut self, tool: ChartTool, cx: &mut Context<Self>) {
        // The page's file picker; the picture comes back through the
        // workspace to `insert_sheet_picture`.
        if let ChartTool::Picture = tool {
            self.host.pick();
            cx.notify();
            return;
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let result = match tool {
            ChartTool::Insert(kind) => {
                match sheet.book.insert_chart(sheet.selection.range(), kind) {
                    Ok(id) => {
                        sheet.object = Some(id);
                        Ok(true)
                    }
                    Err(error) => Err(error),
                }
            }
            ChartTool::Picture => return,
            ChartTool::Shape(preset) => {
                match sheet.book.insert_shape(sheet.selection.active, preset) {
                    Ok(id) => {
                        sheet.object = Some(id);
                        Ok(true)
                    }
                    Err(error) => Err(error),
                }
            }
            ChartTool::Delete => match sheet.object.take() {
                Some(id) => sheet.book.delete_object(id),
                None => Ok(false),
            },
            ChartTool::Kind(kind) => match sheet.selected_chart() {
                Some(id) => sheet.book.set_chart(
                    id,
                    &ChartChange {
                        kind: Some(kind),
                        ..Default::default()
                    },
                ),
                None => Ok(false),
            },
            ChartTool::Legend(legend) => match sheet.selected_chart() {
                Some(id) => sheet.book.set_chart(
                    id,
                    &ChartChange {
                        legend: Some(legend),
                        ..Default::default()
                    },
                ),
                None => Ok(false),
            },
            ChartTool::Ask(field) => {
                if let Some((id, info)) = sheet
                    .selected_chart()
                    .and_then(|id| Some((id, sheet.book.chart(id)?)))
                {
                    sheet.chart_asking = Some(ChartAsk::new(id, field, &info));
                }
                cx.notify();
                return;
            }
        };
        self.edited(result, cx);
    }

    /// A picture picked for Insert ▸ Pictures, put on the sheet at the
    /// active cell, selected.
    pub(crate) fn insert_sheet_picture(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let result = match sheet.book.insert_picture(sheet.selection.active, bytes) {
            Ok(id) => {
                sheet.object = Some(id);
                Ok(true)
            }
            Err(error) => Err(error),
        };
        self.edited(result, cx);
    }

    /// The chart dialog's OK.
    pub(super) fn take_chart_text(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(ask) = sheet.chart_asking.take() else {
            return;
        };
        let result = sheet.book.set_chart(ask.id, &ask.change());
        self.edited(result, cx);
    }

    /// Keys while the chart dialog is open.
    pub(super) fn chart_key(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        let Some(ask) = sheet.chart_asking.as_mut() else {
            return false;
        };
        match ask.key(key, command) {
            ChartKey::Take => {
                self.take_chart_text(cx);
                return true;
            }
            ChartKey::Close => sheet.chart_asking = None,
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
                .text_color(palette::dim())
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
                .hover(|item| item.bg(palette::hovered()))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(
                    cx.listener(move |view, _event, _window, cx| view.tool(Tool::Chart(tool), cx)),
                )
        };
        let mut menu = div().w(px(220.0)).py_1();
        if self.sheet_ref().and_then(Sheet::selected_chart).is_some() {
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
                .child(div().h(px(1.0)).my_1().bg(palette::border()))
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
            menu = menu
                .child(div().h(px(1.0)).my_1().bg(palette::border()))
                .child(item(
                    ("chart-picture", 0),
                    "sheet.insert_picture",
                    ChartTool::Picture,
                    cx,
                ))
                .child(heading("sheet.shapes"))
                .child(
                    div().flex().flex_wrap().text_xs().children(
                        SHAPES
                            .into_iter()
                            .enumerate()
                            .map(|(index, (preset, label))| {
                                item(("chart-shape", index), label, ChartTool::Shape(preset), cx)
                                    .w(px(110.0))
                                    .px_2()
                                    .truncate()
                            }),
                    ),
                );
        }
        menu
    }

    /// The selected object's frame and handles, over the grid — where a
    /// drag has it, while one is on.
    pub(super) fn object_frame(&self) -> Option<gpui::Div> {
        let sheet = self.sheet_ref()?;
        let id = sheet.object?;
        let zoom = self.shown_zoom;
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        let mut rect = sheet.book.object_rect(id, scroll)?;
        if let Some(SheetDrag::Object { bounds, now, .. }) = sheet.drag {
            rect = (
                rect.0 + now.0 - bounds.0,
                rect.1 + now.1 - bounds.1,
                now.2,
                now.3,
            );
        }
        let zoom = f64::from(zoom);
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
                .border_color(palette::accent())
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
                .border_color(palette::accent())
                .children(handles),
        )
    }

    /// Chart Title… and Select Data…: the text, OK and Cancel.
    pub(super) fn chart_dialog(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let ask = self.sheet_ref()?.chart_asking.as_ref()?;
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
                .border_color(palette::border())
                .cursor_pointer()
                .when(primary, |button| {
                    button.bg(palette::accent()).text_color(gpui::white())
                })
                .hover(|button| button.opacity(0.85))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if take {
                        view.take_chart_text(cx);
                    } else {
                        if let Some(sheet) = view.sheet_mut() {
                            sheet.chart_asking = None;
                        }
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
            .border_color(palette::border())
            .bg(palette::panel())
            .shadow_lg()
            .text_xs()
            .text_color(palette::text())
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text_sm().child(i18n::text(ask.field.label_key())))
            .child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(palette::accent())
                    .bg(palette::canvas())
                    .child(caret_line(&ask.text, palette::accent().into(), false)),
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
                .top(px(40.0))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(panel),
        )
    }
}

/// Where an object drag that started at `start` has got to at `(x, y)`.
pub(super) fn object_drag(
    handle: Option<Handle>,
    start: (f64, f64),
    bounds: (f64, f64, f64, f64),
    x: f64,
    y: f64,
) -> (f64, f64, f64, f64) {
    dragged(bounds, handle, (x - start.0, y - start.1))
}

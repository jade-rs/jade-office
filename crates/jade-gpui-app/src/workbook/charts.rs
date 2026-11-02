//! Charts and other drawing objects on the sheet: a click selects one and
//! shows its frame and handles, its border drags it and a handle resizes
//! it, Delete removes it; the toolbar's chart button inserts a chart over
//! the selection, or — with a chart selected — changes its type, legend,
//! title and data, as Excel's Chart Design does.

use gpui::{div, prelude::*, px, Context, Modifiers, MouseButton, SharedString};
use jade_office_model::chart_form::{FieldValue, Form};
use jade_office_model::charts::{
    dragged, handle_at, AskAction, AskPage, ChartAsk, ChartField, ChartInfo, ChartKey, ChartPick,
    Handle, LegendPosition, SelectData, CHART_GALLERY, LEGENDS,
};
use jade_office_model::text::TextEdit;

use super::{SheetDrag, Tool, WorkbookView, SHEET_TOOLBAR_HEIGHT};
use crate::{i18n, theme};
use jade_office_model::freeform::{FreeformDraft, FreeformStep};
use jade_office_model::sheet::SHAPES;

type View = WorkbookView;
include!("charts_ui.rs");

/// How far a press has to travel before it drags an object, in px.
const DRAG_SLOP: f64 = 2.0;

impl WorkbookView {
    /// A click while a freeform is drawn, at a grid point: a corner more;
    /// back on the first, the shape closed; a double-click ends it open.
    pub(super) fn freeform_click(&mut self, x: f64, y: f64, clicks: usize, cx: &mut Context<Self>) {
        let Some(point) = self.book.sheet_point(x, y, self.sheet_scroll()) else {
            return;
        };
        let Some(draft) = self.freeform.as_mut() else {
            return;
        };
        // A scribble starts where it is pressed and follows the drag.
        if draft.scribble {
            draft.drag(point);
            self.drag = Some(SheetDrag::Scribble);
            cx.notify();
            return;
        }
        match draft.add(point) {
            FreeformStep::Close => self.finish_freeform(true, cx),
            FreeformStep::Continue if clicks >= 2 => self.finish_freeform(false, cx),
            FreeformStep::Continue => cx.notify(),
        }
    }

    /// A scribble's drag at a grid point: a point more along its line.
    pub(super) fn scribble_to(&mut self, x: f64, y: f64, cx: &mut Context<Self>) {
        let Some(point) = self.book.sheet_point(x, y, self.sheet_scroll()) else {
            return;
        };
        if let Some(draft) = self.freeform.as_mut() {
            draft.drag(point);
            cx.notify();
        }
    }

    /// Ends the freeform being drawn: put in, closed or open, when it has
    /// two corners or more, and selected.
    pub(super) fn finish_freeform(&mut self, closed: bool, cx: &mut Context<Self>) {
        let Some(draft) = self.freeform.take() else {
            return;
        };
        self.status = SharedString::from("");
        if !draft.can_finish() {
            cx.notify();
            return;
        }
        let result = match self.book.insert_freeform(&draft.points, closed) {
            Ok(id) => {
                self.object = Some(id);
                Ok(true)
            }
            Err(error) => Err(error),
        };
        self.after_write(result, cx);
    }

    /// Keys while a freeform is drawn: Enter ends it open, Escape drops it.
    pub(super) fn freeform_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        match key {
            "enter" => self.finish_freeform(false, cx),
            "escape" => {
                self.freeform = None;
                self.status = SharedString::from("");
                cx.notify();
            }
            _ => return false,
        }
        true
    }

    /// The freeform being drawn, over the grid.
    pub(super) fn freeform_overlay(&self) -> Option<gpui::Canvas<()>> {
        let draft = self.freeform.as_ref()?;
        let zoom = self.zoom;
        let scroll = self.sheet_scroll();
        let corners: Vec<gpui::Point<gpui::Pixels>> = draft
            .points
            .iter()
            .map(|&(x, y)| {
                let (x, y) = self.book.viewport_point(x, y, scroll);
                gpui::point(px(x as f32 * zoom), px(y as f32 * zoom))
            })
            .collect();
        Some(gpui::canvas(
            |_, _, _| {},
            move |bounds, _, window, _| super::paint_freeform(window, bounds.origin, &corners),
        ))
    }

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
            ChartTool::Freeform | ChartTool::Scribble => {
                let scribble = matches!(tool, ChartTool::Scribble);
                self.object = None;
                self.freeform = Some(if scribble {
                    FreeformDraft::scribble()
                } else {
                    FreeformDraft::default()
                });
                self.status = i18n::text(if scribble {
                    "sheet.scribble_hint"
                } else {
                    "sheet.freeform_hint"
                })
                .into();
                cx.notify();
                return;
            }
            ChartTool::Shape(preset) => match self.book.insert_shape(self.active(), preset) {
                Ok(id) => {
                    self.object = Some(id);
                    Ok(true)
                }
                Err(error) => Err(error),
            },
            ChartTool::Insert(pick) => match self.book.insert_chart_as(self.range(), pick) {
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
            ChartTool::Kind(pick) => match self.selected_chart() {
                Some(id) => self.book.set_chart_type(id, pick),
                None => Ok(false),
            },
            ChartTool::SwitchRows => match self.selected_chart() {
                Some(id) => self.book.set_chart(
                    id,
                    &jade_office_model::charts::ChartChange {
                        switch_rows: true,
                        ..Default::default()
                    },
                ),
                None => Ok(false),
            },
            ChartTool::SelectData => {
                if let Some((id, info)) = self
                    .selected_chart()
                    .and_then(|id| Some((id, self.book.chart(id)?)))
                {
                    self.chart_asking = Some(ChartAsk::select_data(id, &info));
                }
                cx.notify();
                return;
            }
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

    /// A click, Enter or Escape in the chart dialog.
    pub(super) fn chart_act(&mut self, action: AskAction, cx: &mut Context<Self>) {
        let Some(mut ask) = self.chart_asking.take() else {
            return;
        };
        let result = match ask.act(&mut self.book, action) {
            Ok(done) => {
                if !done.closed {
                    self.chart_asking = Some(ask);
                }
                Ok(done.wrote)
            }
            Err(error) => {
                self.chart_asking = Some(ask);
                Err(error)
            }
        };
        self.after_write(result, cx);
    }

    fn chart_selection(&self) -> Option<u32> {
        self.selected_chart()
    }

    fn chart_info(&self, id: u32) -> Option<ChartInfo> {
        self.book.chart(id)
    }

    fn chart_ask(&self) -> Option<&ChartAsk> {
        self.chart_asking.as_ref()
    }

    fn chart_dialog_top(&self) -> f32 {
        SHEET_TOOLBAR_HEIGHT + 60.0
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
                self.chart_act(AskAction::Take, cx);
                return true;
            }
            ChartKey::Close => {
                self.chart_act(AskAction::Cancel, cx);
                return true;
            }
            ChartKey::Handled => {}
            ChartKey::Unhandled => return false,
        }
        cx.notify();
        true
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
}

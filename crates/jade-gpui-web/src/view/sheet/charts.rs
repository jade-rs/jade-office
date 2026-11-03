//! Charts and other drawing objects on the sheet: a click selects one and
//! shows its frame and handles, its border drags it and a handle resizes
//! it, Delete removes it; the toolbar's chart button inserts a chart over
//! the selection, or — with a chart selected — changes its type, legend,
//! title and data, as Excel's Chart Design does.

use jade_web::chart_form::{FieldValue, Form};
use jade_web::charts::{
    dragged, handle_at, AskAction, AskPage, ChartAsk, ChartChange, ChartField, ChartInfo, ChartKey,
    ChartPick, Handle, LegendPosition, SelectData, CHART_GALLERY, LEGENDS,
};
use jade_web::text::TextEdit;

use super::*;
use jade_web::freeform::{FreeformDraft, FreeformStep};
use jade_web::sheet::SHAPES;

type View = DocView;
use crate::palette as theme;
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../jade-gpui-app/src/workbook/charts_ui.rs"
));

/// How far a press has to travel before it drags an object, in px.
const DRAG_SLOP: f64 = 2.0;

impl Sheet {
    /// The selected object, when it is a chart.
    pub(super) fn selected_chart(&self) -> Option<u32> {
        self.object.filter(|id| self.book.chart(*id).is_some())
    }
}

impl DocView {
    /// A click while a freeform is drawn, at a grid point: a corner more;
    /// back on the first, the shape closed; a double-click ends it open.
    pub(super) fn freeform_click(&mut self, x: f64, y: f64, clicks: usize, cx: &mut Context<Self>) {
        let zoom = self.shown_zoom;
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        let Some(point) = sheet.book.sheet_point(x, y, scroll) else {
            return;
        };
        let Some(draft) = sheet.freeform.as_mut() else {
            return;
        };
        // A scribble starts where it is pressed and follows the drag.
        if draft.scribble {
            draft.drag(point);
            sheet.drag = Some(SheetDrag::Scribble);
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
        let zoom = self.shown_zoom;
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        let Some(point) = sheet.book.sheet_point(x, y, scroll) else {
            return;
        };
        if let Some(draft) = sheet.freeform.as_mut() {
            draft.drag(point);
            cx.notify();
        }
    }

    /// Ends the freeform being drawn: put in, closed or open, when it has
    /// two corners or more, and selected.
    pub(super) fn finish_freeform(&mut self, closed: bool, cx: &mut Context<Self>) {
        self.status = SharedString::default();
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(draft) = sheet.freeform.take() else {
            return;
        };
        if !draft.can_finish() {
            cx.notify();
            return;
        }
        let result = match sheet.book.insert_freeform(&draft.points, closed) {
            Ok(id) => {
                sheet.object = Some(id);
                Ok(true)
            }
            Err(error) => Err(error),
        };
        self.edited(result, cx);
    }

    /// Keys while a freeform is drawn: Enter ends it open, Escape drops it.
    pub(super) fn freeform_key(&mut self, key: &str, cx: &mut Context<Self>) -> bool {
        match key {
            "enter" => self.finish_freeform(false, cx),
            "escape" => {
                if let Some(sheet) = self.sheet_mut() {
                    sheet.freeform = None;
                }
                self.status = SharedString::default();
                cx.notify();
            }
            _ => return false,
        }
        true
    }

    /// The freeform being drawn, over the grid.
    pub(super) fn freeform_overlay(&self) -> Option<gpui::Canvas<()>> {
        let sheet = self.sheet_ref()?;
        let draft = sheet.freeform.as_ref()?;
        let zoom = self.shown_zoom;
        let scroll = Self::sheet_scroll(&sheet.scroll, zoom);
        let corners: Vec<Point<Pixels>> = draft
            .points
            .iter()
            .map(|&(x, y)| {
                let (x, y) = sheet.book.viewport_point(x, y, scroll);
                gpui::point(px(x as f32 * zoom), px(y as f32 * zoom))
            })
            .collect();
        Some(gpui::canvas(
            |_, _, _| {},
            move |bounds, _, window, _| paint_freeform(window, bounds.origin, &corners),
        ))
    }

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
        if let ChartTool::Freeform | ChartTool::Scribble = tool {
            let scribble = matches!(tool, ChartTool::Scribble);
            if let Some(sheet) = self.sheet_mut() {
                sheet.object = None;
                sheet.freeform = Some(if scribble {
                    FreeformDraft::scribble()
                } else {
                    FreeformDraft::default()
                });
            }
            self.status = i18n::text(if scribble {
                "sheet.scribble_hint"
            } else {
                "sheet.freeform_hint"
            })
            .into();
            cx.notify();
            return;
        }
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let result = match tool {
            ChartTool::Freeform | ChartTool::Scribble => return,
            ChartTool::Insert(pick) => {
                match sheet.book.insert_chart_as(sheet.selection.range(), pick) {
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
            ChartTool::Kind(pick) => match sheet.selected_chart() {
                Some(id) => sheet.book.set_chart_type(id, pick),
                None => Ok(false),
            },
            ChartTool::SwitchRows => match sheet.selected_chart() {
                Some(id) => sheet.book.set_chart(
                    id,
                    &ChartChange {
                        switch_rows: true,
                        ..Default::default()
                    },
                ),
                None => Ok(false),
            },
            ChartTool::SelectData => {
                if let Some((id, info)) = sheet
                    .selected_chart()
                    .and_then(|id| Some((id, sheet.book.chart(id)?)))
                {
                    sheet.chart_asking = Some(ChartAsk::select_data(id, &info));
                }
                cx.notify();
                return;
            }
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

    /// A click, Enter or Escape in the chart dialog.
    pub(super) fn chart_act(&mut self, action: AskAction, cx: &mut Context<Self>) {
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        let Some(mut ask) = sheet.chart_asking.take() else {
            return;
        };
        let result = match ask.act(&mut sheet.book, action) {
            Ok(done) => {
                if !done.closed {
                    sheet.chart_asking = Some(ask);
                }
                Ok(done.wrote)
            }
            Err(error) => {
                sheet.chart_asking = Some(ask);
                Err(error)
            }
        };
        self.edited(result, cx);
    }

    fn chart_selection(&self) -> Option<u32> {
        self.sheet_ref().and_then(Sheet::selected_chart)
    }

    fn chart_info(&self, id: u32) -> Option<ChartInfo> {
        self.sheet_ref()?.book.chart(id)
    }

    fn chart_ask(&self) -> Option<&ChartAsk> {
        self.sheet_ref()?.chart_asking.as_ref()
    }

    fn chart_dialog_top(&self) -> f32 {
        40.0
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

//! The table tools over a Word document's pages: the controls of the
//! テーブル デザイン and テーブル レイアウト tabs, the open table dialog, and the
//! mouse in a table — a border dragged, a row, a column, a cell or the
//! table selected from its edge, a drag across cells, the pencil and the
//! eraser. What each does is `jade-office-model`'s `TableTools`, the code the
//! native app runs.

use jade_web::word::table::WordTables;
use jade_web::word::table_ui::{TableAction, TableDrag, TableOutcome, TableTools};
use jade_web::word::WordSession;

use super::*;

impl DocView {
    /// The table tools of the Word document in front.
    pub(crate) fn table_tools_mut(&mut self) -> Option<&mut TableTools> {
        match &mut self.document {
            Document::Pages { tables, .. } => Some(tables),
            _ => None,
        }
    }

    fn table_tools(&self) -> Option<&TableTools> {
        match &self.document {
            Document::Pages { tables, .. } => Some(tables),
            _ => None,
        }
    }

    /// Whether a table dialog is open, taking the keyboard.
    pub(super) fn table_dialog_open(&self) -> bool {
        self.table_tools()
            .is_some_and(|tools| tools.dialog.is_some())
    }

    pub(crate) fn close_table_dialog(&mut self) {
        if let Some(tools) = self.table_tools_mut() {
            tools.dialog = None;
        }
    }

    /// Runs `step` on the document and its table tools, and shows what it
    /// did.
    fn with_tables(
        &mut self,
        cx: &mut Context<Self>,
        step: impl FnOnce(&mut jade_web::editing::EditableDocument, &mut TableTools) -> TableOutcome,
    ) -> bool {
        let Document::Pages {
            document, tables, ..
        } = &mut self.document
        else {
            return false;
        };
        let outcome = step(document, tables);
        let mine = outcome != TableOutcome::NotMine;
        match outcome {
            TableOutcome::NotMine | TableOutcome::Redraw => cx.notify(),
            TableOutcome::Edited(result) => {
                if let Document::Pages { document, .. } = &mut self.document {
                    if let Ok(Some((paragraph, offset))) = result {
                        document.place_caret(paragraph, offset);
                    }
                }
                self.refresh_repainted();
                self.reveal_caret();
                self.word_result(result.map(|_| ()), cx);
            }
            TableOutcome::Selected(result) => {
                if let Err(error) = result {
                    self.status = error.into();
                }
                self.refresh_repainted();
                cx.notify();
            }
        }
        mine
    }

    /// A table control was clicked.
    pub(super) fn table_action(&mut self, action: TableAction, cx: &mut Context<Self>) {
        self.with_tables(cx, |document, tables| tables.action(document, action));
    }

    /// OK in the open dialog.
    pub(crate) fn apply_table_dialog(&mut self, cx: &mut Context<Self>) {
        self.with_tables(cx, |document, tables| tables.apply_dialog(document));
    }

    /// The keys an open dialog takes: Enter, Escape, Backspace, and the
    /// arrows it leaves the pages alone for. Text comes through the input
    /// handler (`table_dialog_typed`).
    pub(super) fn table_dialog_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.table_dialog_open() {
            return false;
        }
        let keystroke = &event.keystroke;
        let key = keystroke.key.clone();
        let modifiers = keystroke.modifiers;
        if !matches!(key.as_str(), "escape" | "enter" | "return" | "backspace") {
            // Text goes to the box through the input handler; the arrows and
            // the shortcuts do nothing to the pages behind the dialog.
            return keystroke.key_char.is_none() || modifiers.platform || modifiers.control;
        }
        self.with_tables(cx, |document, tables| {
            tables
                .dialog_key(document, &key, None)
                .unwrap_or(TableOutcome::NotMine)
        });
        true
    }

    /// Text typed while a dialog is open goes into its box.
    pub(super) fn table_dialog_typed(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        let Some(dialog) = self
            .table_tools_mut()
            .and_then(|tools| tools.dialog.as_mut())
        else {
            return false;
        };
        dialog.type_text(text);
        cx.notify();
        true
    }

    /// A press at (`x`, `y`) on page `index`: whether the table took it.
    pub(super) fn table_mouse_down(
        &mut self,
        index: usize,
        point: (f64, f64),
        click_count: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        self.with_tables(cx, |document, tables| {
            tables.mouse_down(document, index, point, click_count)
        })
    }

    /// The page under a window point, and the point on it in layout pixels.
    fn table_point(&self, position: gpui::Point<Pixels>) -> Option<(usize, (f64, f64))> {
        let Document::Pages {
            page_bounds, pages, ..
        } = &self.document
        else {
            return None;
        };
        let index = page_bounds
            .borrow()
            .iter()
            .take(pages.len())
            .position(|bounds| bounds.is_some_and(|bounds| bounds.contains(&position)))?;
        Some((index, self.page_point(index, position)?))
    }

    /// The pointer moved: with the button down a border or the pencil
    /// follows it and a drag across cells selects them; without, what it
    /// is over decides its shape. Whether the table took the move.
    pub(super) fn table_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let point = self.table_point(event.position).map(|(_, point)| point);
        if event.pressed_button != Some(MouseButton::Left) {
            let Document::Pages {
                document, tables, ..
            } = &mut self.document
            else {
                return false;
            };
            if tables.hover(document.as_ref(), point) {
                cx.notify();
            }
            return false;
        }
        let anchor = match &self.document {
            Document::Pages {
                document,
                dragging: true,
                ..
            } => document
                .selection()
                .map(|(anchor, _)| anchor)
                .or_else(|| document.caret()),
            _ => None,
        };
        self.with_tables(cx, |document, tables| {
            tables.mouse_drag(document, point, anchor)
        })
    }

    /// The button came up: a dragged border lands, a pencil stroke draws.
    pub(super) fn table_mouse_up(&mut self, cx: &mut Context<Self>) {
        let dragging = self.table_tools().is_some_and(|tools| tools.drag.is_some());
        if dragging {
            self.with_tables(cx, |document, tables| {
                tables.mouse_up(document, |document, x, y| document.position_at(x, y))
            });
        }
    }

    /// What a drag in a table shows over page `index`, zoomed: the guide
    /// where a border will land, the box the pencil draws — or, with no
    /// drag, the caret table's handle.
    pub(super) fn table_overlay(&self, index: usize, zoom: f32) -> Option<gpui::Div> {
        let Document::Pages {
            document, tables, ..
        } = &self.document
        else {
            return None;
        };
        let local = |y: f64| {
            let (page, offset) = document.page_and_offset(y);
            (page, offset as f32)
        };
        let Some((drag, drag_page, from, to)) = tables.drag else {
            // The handle at the caret table's top-left corner.
            let caret = document.caret()?;
            let table_id = document
                .cell_id_for_paragraph(caret.0)
                .and_then(|id| document.cell_layout(id))
                .map(|cell| cell.table_id)?;
            let geometry = document.table_geometry(table_id)?;
            let (page, top) = local(geometry.top());
            if page != index {
                return None;
            }
            return Some(
                div()
                    .absolute()
                    .left(px((geometry.left() as f32 - 20.0) * zoom))
                    .top(px((top - 20.0) * zoom))
                    .size(px(12.0 * zoom))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(palette::dim())
                    .bg(palette::panel())
                    .text_size(px(9.0 * zoom))
                    .text_color(palette::dim())
                    .child("+"),
            );
        };
        let (from_page, from_y) = local(from.1);
        let (_, to_y) = local(to.1);
        if from_page != index || drag_page != index {
            return None;
        }
        Some(match drag {
            TableDrag::Column { table_id, .. } => {
                let geometry = document.table_geometry(table_id)?;
                let (top_page, top) = local(geometry.top());
                let bottom = geometry.rows.last().map_or(geometry.top(), |row| row.1);
                let (_, bottom) = local(bottom);
                let top = if top_page == index { top } else { 0.0 };
                div()
                    .absolute()
                    .left(px(to.0 as f32 * zoom))
                    .top(px(top * zoom))
                    .w(px(1.0))
                    .h(px((bottom - top).max(4.0) * zoom))
                    .bg(palette::accent())
            }
            TableDrag::Row { table_id, .. } => {
                let geometry = document.table_geometry(table_id)?;
                div()
                    .absolute()
                    .left(px(geometry.left() as f32 * zoom))
                    .top(px(to_y * zoom))
                    .w(px((geometry.right() - geometry.left()) as f32 * zoom))
                    .h(px(1.0))
                    .bg(palette::accent())
            }
            TableDrag::Stroke => {
                let (left, right) = (from.0.min(to.0) as f32, from.0.max(to.0) as f32);
                let (top, bottom) = (from_y.min(to_y), from_y.max(to_y));
                div()
                    .absolute()
                    .left(px(left * zoom))
                    .top(px(top * zoom))
                    .w(px(((right - left) * zoom).max(1.0)))
                    .h(px(((bottom - top) * zoom).max(1.0)))
                    .border_1()
                    .border_color(palette::accent())
            }
        })
    }

    /// The pointer's shape over the pages.
    pub(super) fn table_cursor(&self) -> gpui::CursorStyle {
        self.table_tools()
            .map(crate::word_table::cursor_style)
            .unwrap_or(gpui::CursorStyle::IBeam)
    }

    /// The open table dialog, over the pages.
    pub(super) fn table_dialog_element(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let dialog = self.table_tools()?.dialog.as_ref()?;
        Some(crate::word_table::dialog_element(dialog, cx))
    }

    /// What the table tabs show, read only while one of them is showing.
    pub(super) fn table_tab_state(&self) -> Option<jade_web::word::table_ui::TableTabState> {
        let Document::Pages {
            document,
            tables,
            ribbon,
            ..
        } = &self.document
        else {
            return None;
        };
        matches!(
            ribbon.tab,
            ribbon::Tab::TableDesign | ribbon::Tab::TableLayout
        )
        .then(|| tables.tab_state(document.as_ref()))
    }
}

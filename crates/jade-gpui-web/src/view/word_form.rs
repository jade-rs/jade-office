//! Draws a dialog `jade-office-model` describes as a [`FormView`] — fields,
//! choices, check boxes, lists, a grid, buttons — over the window, and turns
//! clicks and keys on it into [`FormEvent`]s for whoever owns the dialog.
//! The browser shell's copy of the native app's `word_form`: the content
//! control and mail-merge dialogs are drawn this way in both.

use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, ClickEvent, Context, ElementId,
    MouseButton, SharedString, Window,
};
use jade_web::word::form_view::{
    FieldView, FormButton, FormEvent, FormId, FormRow, FormView, ToolGroup, ToolItem,
};

use crate::palette as theme;
use crate::view::DocView as DocumentView;

/// What a dialog's owner does with an event.
pub type Handler = fn(&mut DocumentView, FormEvent, &mut Context<DocumentView>);

fn element_id(id: FormId, part: &'static str) -> ElementId {
    ElementId::Name(SharedString::from(format!("form-{}-{}-{part}", id.0, id.1)))
}

/// A click handler that hands `event` to the dialog's owner.
macro_rules! listener {
    ($event:expr, $handler:expr, $cx:expr) => {{
        let event: FormEvent = $event;
        let handler: Handler = $handler;
        $cx.listener(move |view, _: &ClickEvent, _, cx| handler(view, event.clone(), cx))
    }};
}

/// A key while a dialog is open, as the event it is: Tab, Enter, Escape.
pub fn key_event(key: &str, shift: bool) -> Option<FormEvent> {
    match key {
        "tab" => Some(FormEvent::Tab(shift)),
        "enter" | "return" => Some(FormEvent::Enter),
        "escape" => Some(FormEvent::Escape),
        _ => None,
    }
}

fn chip(id: ElementId, label: String, on: bool, enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when(on, |chip| chip.bg(theme::selected()))
        .when(!enabled, |chip| chip.text_color(theme::faint()))
        .when(enabled, |chip| {
            chip.cursor_pointer()
                .hover(|chip| chip.bg(theme::hovered()))
        })
        .child(label)
}

fn label(text: &str) -> gpui::Div {
    div()
        .w(px(120.0))
        .flex_none()
        .text_color(theme::dim())
        .child(text.to_string())
}

fn field_text(value: &FieldView, focused: bool) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .when(focused && value.whole, |row| {
            row.child(div().bg(theme::selected()).child(value.text()))
                .child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
        })
        .when(!(focused && value.whole), |row| {
            row.child(value.before.clone())
                .when(focused, |row| {
                    row.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                })
                .child(value.after.clone())
        })
}

fn button_row(
    buttons: &[FormButton],
    handler: Handler,
    cx: &mut Context<DocumentView>,
) -> gpui::Div {
    div()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(buttons.iter().map(|button| {
            let element = chip(
                element_id(button.id, "button"),
                button.label.clone(),
                button.primary,
                button.enabled,
            );
            if button.enabled {
                element
                    .on_click(listener!(FormEvent::Click(button.id), handler, cx))
                    .into_any_element()
            } else {
                element.into_any_element()
            }
        }))
}

fn row(row_view: &FormRow, handler: Handler, cx: &mut Context<DocumentView>) -> AnyElement {
    match row_view {
        FormRow::Heading(text) => div()
            .pt_1()
            .text_color(theme::faint())
            .border_b_1()
            .border_color(theme::border())
            .child(text.clone())
            .into_any_element(),
        FormRow::Note(text) => div()
            .text_color(theme::dim())
            .child(text.clone())
            .into_any_element(),
        FormRow::Preview(lines) => div()
            .flex()
            .flex_col()
            .p_2()
            .min_h(px(48.0))
            .rounded_sm()
            .border_1()
            .border_color(theme::border())
            .bg(theme::paper())
            .text_color(gpui::rgb(0x222222))
            .children(lines.iter().map(|line| div().child(line.clone())))
            .into_any_element(),
        FormRow::Field {
            id,
            label: text,
            value,
            focused,
            width,
        } => div()
            .flex()
            .items_center()
            .gap_2()
            .child(label(text))
            .child(
                div()
                    .id(element_id(*id, "field"))
                    .w(px(*width))
                    .h(px(22.0))
                    .px_1()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(if *focused {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(theme::canvas())
                    .cursor_text()
                    .on_click(listener!(FormEvent::Click(*id), handler, cx))
                    .child(field_text(value, *focused)),
            )
            .into_any_element(),
        FormRow::Choice {
            id,
            label: text,
            options,
            selected,
        } => div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_1()
            .child(label(text))
            .children(options.iter().enumerate().map(|(index, option)| {
                chip(
                    ElementId::Name(SharedString::from(format!(
                        "form-{}-{}-choice-{index}",
                        id.0, id.1
                    ))),
                    option.clone(),
                    *selected == Some(index),
                    true,
                )
                .on_click(listener!(FormEvent::Choose(*id, index), handler, cx))
            }))
            .into_any_element(),
        FormRow::TextArea {
            id,
            label: text,
            value,
            focused,
            width,
            lines,
        } => {
            let full = value.text();
            let caret_line = value.before.matches('\n').count();
            let before_last = value.before.rsplit('\n').next().unwrap_or("").to_string();
            let after_first = value.after.split('\n').next().unwrap_or("").to_string();
            div()
                .flex()
                .items_start()
                .gap_2()
                .child(label(text))
                .child(
                    div()
                        .id(element_id(*id, "area"))
                        .w(px(*width))
                        .min_h(px(18.0 * (*lines).max(1) as f32 + 6.0))
                        .px_1()
                        .py_0p5()
                        .flex()
                        .flex_col()
                        .rounded_sm()
                        .border_1()
                        .border_color(if *focused {
                            theme::accent()
                        } else {
                            theme::border()
                        })
                        .bg(theme::canvas())
                        .cursor_text()
                        .on_click(listener!(FormEvent::Click(*id), handler, cx))
                        .children(full.split('\n').enumerate().map(|(index, line)| {
                            if *focused && index == caret_line {
                                div()
                                    .flex()
                                    .items_center()
                                    .h(px(18.0))
                                    .child(before_last.clone())
                                    .child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                                    .child(after_first.clone())
                            } else {
                                div().h(px(18.0)).child(line.to_string())
                            }
                        })),
                )
                .into_any_element()
        }
        FormRow::Check {
            id,
            label: text,
            on,
        } => div()
            .id(element_id(*id, "check"))
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer()
            .on_click(listener!(FormEvent::Toggle(*id), handler, cx))
            .child(
                div()
                    .size(px(12.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(if *on {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .when(*on, |check| check.bg(theme::accent())),
            )
            .child(text.clone())
            .into_any_element(),
        FormRow::List {
            id,
            label: text,
            items,
            selected,
            rows,
        } => div()
            .flex()
            .gap_2()
            .when(!text.is_empty(), |list| list.child(label(text)))
            .child(
                div()
                    .id(element_id(*id, "list"))
                    .flex_1()
                    .min_w(px(160.0))
                    .h(px(20.0 * (*rows).max(1) as f32 + 4.0))
                    .overflow_y_scroll()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::canvas())
                    .children(items.iter().enumerate().map(|(index, item)| {
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "form-{}-{}-item-{index}",
                                id.0, id.1
                            ))))
                            .h(px(20.0))
                            .px_1()
                            .flex()
                            .items_center()
                            .whitespace_nowrap()
                            .cursor_pointer()
                            .when(*selected == Some(index), |item| item.bg(theme::selected()))
                            .hover(|item| item.bg(theme::hovered()))
                            .on_click(listener!(FormEvent::Choose(*id, index), handler, cx))
                            .child(item.clone())
                    })),
            )
            .into_any_element(),
        FormRow::Grid {
            id,
            headers,
            rows,
            selected,
            editing,
            sorted,
        } => {
            let checks = rows.iter().any(|row| row.checked.is_some());
            let cell_width = 120.0;
            let header = div()
                .flex()
                .bg(theme::panel())
                .border_b_1()
                .border_color(theme::border())
                .when(checks, |header| header.child(div().w(px(24.0)).flex_none()))
                .children(headers.iter().enumerate().map(|(column, text)| {
                    let arrow = match sorted {
                        Some((sorted, up)) if *sorted == column => {
                            if *up {
                                " ▲"
                            } else {
                                " ▼"
                            }
                        }
                        _ => "",
                    };
                    div()
                        .id(ElementId::Name(SharedString::from(format!(
                            "form-{}-{}-header-{column}",
                            id.0, id.1
                        ))))
                        .w(px(cell_width))
                        .flex_none()
                        .px_1()
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .cursor_pointer()
                        .hover(|cell| cell.bg(theme::hovered()))
                        .on_click(listener!(FormEvent::Header(*id, column), handler, cx))
                        .child(format!("{text}{arrow}"))
                }));
            let body = rows.iter().enumerate().map(|(row_index, grid_row)| {
                div()
                    .flex()
                    .border_b_1()
                    .border_color(theme::border())
                    .children(grid_row.checked.map(|checked| {
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "form-{}-{}-row-check-{row_index}",
                                id.0, id.1
                            ))))
                            .w(px(24.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(listener!(FormEvent::RowCheck(*id, row_index), handler, cx))
                            .child(
                                div()
                                    .size(px(12.0))
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(if checked {
                                        theme::accent()
                                    } else {
                                        theme::border()
                                    })
                                    .when(checked, |check| check.bg(theme::accent())),
                            )
                    }))
                    .children(grid_row.cells.iter().enumerate().map(|(column, text)| {
                        let is_selected = *selected == Some((row_index, column));
                        let edit = editing
                            .as_ref()
                            .filter(|(row, col, _)| *row == row_index && *col == column)
                            .map(|(_, _, value)| value.clone());
                        div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "form-{}-{}-cell-{row_index}-{column}",
                                id.0, id.1
                            ))))
                            .w(px(cell_width))
                            .h(px(20.0))
                            .flex_none()
                            .px_1()
                            .flex()
                            .items_center()
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .border_r_1()
                            .border_color(theme::border())
                            .when(is_selected, |cell| cell.bg(theme::selected()))
                            .cursor_pointer()
                            .on_click(listener!(
                                FormEvent::Cell(*id, row_index, column),
                                handler,
                                cx
                            ))
                            .map(|cell| match edit {
                                Some(value) => cell.child(field_text(&value, true)),
                                None => cell.child(text.clone()),
                            })
                    }))
            });
            div()
                .id(element_id(*id, "grid"))
                .max_h(px(260.0))
                .max_w(px(760.0))
                .overflow_scroll()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .bg(theme::canvas())
                .child(header)
                .children(body)
                .into_any_element()
        }
        FormRow::Buttons(buttons) => button_row(buttons, handler, cx).into_any_element(),
        FormRow::Columns(columns) => div()
            .flex()
            .gap_4()
            .children(columns.iter().map(|column| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(column.iter().map(|inner| row(inner, handler, cx)))
            }))
            .into_any_element(),
    }
}

/// The dialog over the window: a backdrop that takes clicks, the dialog in
/// the middle of it.
pub fn overlay(
    form: &FormView,
    window: &mut Window,
    handler: Handler,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let viewport = window.viewport_size();
    deferred(
        anchored().position(point(px(0.0), px(0.0))).child(
            div()
                .id("form-backdrop")
                .w(viewport.width)
                .h(viewport.height)
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000040))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("form-dialog")
                        .w(px(form.width))
                        .max_h(viewport.height - px(40.0))
                        .overflow_y_scroll()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .rounded_lg()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::panel())
                        .shadow_lg()
                        .font_family(crate::UI_FONT)
                        .text_xs()
                        .text_color(theme::text())
                        .child(div().text_sm().child(form.title.clone()))
                        .children(form.rows.iter().map(|inner| row(inner, handler, cx)))
                        .children(
                            form.problem.clone().map(|problem| {
                                div().text_color(gpui::rgb(0xD13438)).child(problem)
                            }),
                        )
                        .child(div().flex().justify_end().child(button_row(
                            &form.buttons,
                            handler,
                            cx,
                        ))),
                ),
        ),
    )
    .with_priority(3)
    .into_any_element()
}

/// A ribbon tab's groups described as [`ToolGroup`]s: buttons, menus that
/// drop their entries under them, and text.
pub fn tool_groups(
    groups: &[ToolGroup],
    handler: Handler,
    cx: &mut Context<DocumentView>,
) -> Vec<AnyElement> {
    groups
        .iter()
        .map(|group| {
            let rows: Vec<AnyElement> = group
                .rows
                .iter()
                .map(|row| {
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .children(row.iter().map(|item| tool_item(item, handler, cx)))
                        .into_any_element()
                })
                .collect();
            tool_group(&group.label, rows)
        })
        .collect()
}

fn tool_group(caption: &str, rows: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .items_center()
        .gap_1()
        .px_3()
        .border_r_1()
        .border_color(theme::border())
        .child(div().flex().flex_col().gap_1().children(rows))
        .child(
            div()
                .text_xs()
                .text_color(theme::faint())
                .child(caption.to_string()),
        )
        .into_any_element()
}

fn tool_item(item: &ToolItem, handler: Handler, cx: &mut Context<DocumentView>) -> AnyElement {
    let button = |id: ElementId, label: String, title: String, on: bool, enabled: bool| {
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .h(px(26.0))
            .px_2()
            .rounded_md()
            .text_xs()
            .whitespace_nowrap()
            .text_color(if enabled {
                theme::text()
            } else {
                theme::faint()
            })
            .when(on, |button| button.bg(theme::selected()))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(|button| button.bg(theme::hovered()))
            })
            .tooltip(crate::tooltip::Tooltip::text(title))
            .child(label)
    };
    match item {
        ToolItem::Button {
            id,
            label,
            title,
            on,
            enabled,
        } => {
            let element = button(
                element_id(*id, "tool"),
                label.clone(),
                title.clone(),
                *on,
                *enabled,
            );
            if *enabled {
                element
                    .on_click(listener!(FormEvent::Click(*id), handler, cx))
                    .into_any_element()
            } else {
                element.into_any_element()
            }
        }
        ToolItem::Text(text) => div()
            .px_1()
            .text_xs()
            .text_color(theme::dim())
            .whitespace_nowrap()
            .child(text.clone())
            .into_any_element(),
        ToolItem::Menu {
            id,
            label,
            title,
            enabled,
            open,
            items,
        } => {
            let trigger = button(
                element_id(*id, "menu"),
                format!("{label} ▼"),
                title.clone(),
                *open,
                *enabled,
            );
            let trigger = if *enabled {
                trigger
                    .on_click(listener!(FormEvent::Click(*id), handler, cx))
                    .into_any_element()
            } else {
                trigger.into_any_element()
            };
            let menu = open.then(|| {
                div()
                    .id("tool-menu")
                    .absolute()
                    .top(px(28.0))
                    .left_0()
                    .min_w(px(200.0))
                    .max_h(px(360.0))
                    .overflow_y_scroll()
                    .p_1()
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::panel())
                    .shadow_lg()
                    .occlude()
                    .children(items.iter().enumerate().map(|(index, entry)| {
                        let row = div()
                            .id(ElementId::Name(SharedString::from(format!(
                                "tool-{}-{}-{index}",
                                id.0, id.1
                            ))))
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .whitespace_nowrap()
                            .when(entry.on, |row| row.bg(theme::selected()))
                            .when(!entry.enabled, |row| row.text_color(theme::faint()))
                            .child(entry.label.clone());
                        if entry.enabled {
                            row.cursor_pointer()
                                .hover(|row| row.bg(theme::hovered()))
                                .on_click(listener!(FormEvent::Choose(*id, index), handler, cx))
                                .into_any_element()
                        } else {
                            row.into_any_element()
                        }
                    }))
            });
            div()
                .relative()
                .child(trigger)
                .children(menu.map(|menu| gpui::deferred(menu).with_priority(2)))
                .into_any_element()
        }
    }
}

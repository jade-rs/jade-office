//! その他の色: the colour wheel Word for Mac opens for the text colour — the
//! hue around the circle, the saturation out from its centre, the
//! brightness on the slider beside it — with the new colour against the
//! current one, the hex code to type, and rows of greys and Word's
//! standard colours. Opened from the ribbon's colour list or the フォント
//! dialog's, which it goes back to. The wheel's arithmetic is
//! `jade-office-model`'s (`color_wheel`).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    canvas, div, fill, point, prelude::*, px, size, AnyElement, Bounds, ClickEvent, Context,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
};
use jade_web::color_wheel::{brightness_at, hsv_to_rgb, wheel_cells, ColorWheel, Hsv};
use jade_web::word::font::{FontDialog, STANDARD_COLORS};

use super::{caption, swatch, Dialog};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

/// The wheel's radius, and the squares it is painted in.
pub(crate) const RADIUS: f32 = 84.0;
const CELL: f32 = 3.0;
const SLIDER_WIDTH: f32 = 18.0;

/// What a drag is moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Drag {
    Wheel,
    Slider,
}

pub struct ColorDialog {
    pub wheel: ColorWheel,
    /// The フォント dialog to go back to, when it opened this.
    pub back: Option<Box<FontDialog>>,
    /// Where the wheel and the slider were last drawn, for the mouse.
    pub(crate) wheel_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub(crate) slider_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub(crate) drag: Option<Drag>,
}

impl ColorDialog {
    fn new(current: Option<String>, back: Option<Box<FontDialog>>) -> Self {
        Self {
            wheel: ColorWheel::new(current.as_deref()),
            back,
            wheel_bounds: Rc::default(),
            slider_bounds: Rc::default(),
            drag: None,
        }
    }

    pub fn for_font(current: Option<String>) -> Self {
        Self::new(current, None)
    }

    pub fn from_font_dialog(font: FontDialog, current: Option<String>) -> Self {
        Self::new(current, Some(Box::new(font)))
    }

    /// The colour OK applies, `RRGGBB`.
    pub fn hex(&self) -> String {
        self.wheel.hex()
    }

    /// The mouse at `position` (window coordinates), on what is dragged.
    pub(crate) fn drag_to(&mut self, drag: Drag, position: gpui::Point<Pixels>) {
        match drag {
            Drag::Wheel => {
                let Some(bounds) = self.wheel_bounds.get() else {
                    return;
                };
                let center = bounds.center();
                let dx = f32::from(position.x - center.x) as f64;
                let dy = f32::from(position.y - center.y) as f64;
                self.wheel.pick_point(dx, dy, RADIUS as f64);
            }
            Drag::Slider => {
                let Some(bounds) = self.slider_bounds.get() else {
                    return;
                };
                let y = f32::from(position.y - bounds.origin.y) as f64;
                let height = f32::from(bounds.size.height) as f64;
                self.wheel.set_brightness(brightness_at(y, height));
            }
        }
    }
}

fn rgb_of((r, g, b): (u8, u8, u8)) -> gpui::Rgba {
    gpui::rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32)
}

/// The dialog's colour, changed by `edit`; the code typed is dropped, so
/// the field shows the colour again.
fn edit(view: &mut DocumentView, edit: impl FnOnce(&mut ColorDialog)) {
    if let Some(Dialog::Color(dialog)) = view.characters.dialog.as_mut() {
        edit(dialog);
    }
    view.characters.field.text.clear();
    view.characters.field.caret = 0;
}

fn start_drag(view: &mut DocumentView, drag: Drag, position: gpui::Point<Pixels>) {
    edit(view, |dialog| {
        dialog.drag = Some(drag);
        dialog.drag_to(drag, position);
    });
}

/// The wheel: painted square by square at the slider's brightness, with a
/// ring where the colour is.
fn wheel(dialog: &ColorDialog, cx: &mut Context<DocumentView>) -> AnyElement {
    let cells = wheel_cells(RADIUS as f64, CELL as f64, dialog.wheel.hsv.value);
    let bounds = Rc::clone(&dialog.wheel_bounds);
    let (mx, my) = dialog.wheel.marker(RADIUS as f64);
    let ring = if dialog.wheel.hsv.value > 0.5 {
        gpui::rgb(0x000000)
    } else {
        gpui::rgb(0xFFFFFF)
    };
    div()
        .id("color-wheel")
        .relative()
        .flex_none()
        .size(px(2.0 * RADIUS))
        .cursor_crosshair()
        .child(
            canvas(
                move |area, _window, _cx| bounds.set(Some(area)),
                move |area, (), window, _cx| {
                    for cell in &cells {
                        let origin = point(
                            area.origin.x + px(cell.x as f32),
                            area.origin.y + px(cell.y as f32),
                        );
                        window.paint_quad(fill(
                            Bounds::new(origin, size(px(CELL), px(CELL))),
                            rgb_of(cell.rgb),
                        ));
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(
            div()
                .absolute()
                .left(px(RADIUS + mx as f32 - 5.0))
                .top(px(RADIUS + my as f32 - 5.0))
                .size(px(10.0))
                .rounded_full()
                .border_2()
                .border_color(ring),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|view, event: &MouseDownEvent, _, cx| {
                start_drag(view, Drag::Wheel, event.position);
                cx.notify();
            }),
        )
        .into_any_element()
}

/// The brightness slider: the wheel's colour at full brightness at the top
/// down to black, with a bar where the brightness is.
fn slider(dialog: &ColorDialog, cx: &mut Context<DocumentView>) -> AnyElement {
    let hsv = dialog.wheel.hsv;
    let top = rgb_of(hsv_to_rgb(Hsv { value: 1.0, ..hsv }));
    let bounds = Rc::clone(&dialog.slider_bounds);
    let height = 2.0 * RADIUS;
    div()
        .id("color-brightness")
        .relative()
        .flex_none()
        .w(px(SLIDER_WIDTH))
        .h(px(height))
        .border_1()
        .border_color(theme::border())
        .cursor_pointer()
        .bg(gpui::linear_gradient(
            180.0,
            gpui::linear_color_stop(top, 0.0),
            gpui::linear_color_stop(gpui::rgb(0x000000), 1.0),
        ))
        .child(
            canvas(
                move |area, _window, _cx| bounds.set(Some(area)),
                |_, (), _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(
            div()
                .absolute()
                .left(px(-3.0))
                .top(px((1.0 - hsv.value as f32) * height - 2.0))
                .w(px(SLIDER_WIDTH + 4.0))
                .h(px(4.0))
                .rounded_sm()
                .border_1()
                .border_color(theme::text())
                .bg(theme::panel()),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|view, event: &MouseDownEvent, _, cx| {
                start_drag(view, Drag::Slider, event.position);
                cx.notify();
            }),
        )
        .into_any_element()
}

/// A row of colours to click.
fn swatches(
    id: &'static str,
    colors: Vec<String>,
    current: &str,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .flex()
        .gap_0p5()
        .children(colors.into_iter().enumerate().map(|(index, color)| {
            let on = color == current;
            swatch((id, index), &color, on).on_click(cx.listener(
                move |view, _: &ClickEvent, _, cx| {
                    edit(view, |dialog| {
                        dialog.wheel.set_hex(&color);
                    });
                    cx.notify();
                },
            ))
        }))
        .into_any_element()
}

pub(super) fn body(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let typed = view.characters.field.text.clone();
    let Some(Dialog::Color(dialog)) = view.characters.dialog.as_mut() else {
        return div().into_any_element();
    };
    // A full code typed moves the wheel to it.
    if !typed.is_empty() {
        dialog.wheel.set_hex(&typed);
    }
    let dialog = &*dialog;
    let new = dialog.hex();
    let current = dialog.wheel.current.clone();
    let greys: Vec<String> = (0..10)
        .map(|step| {
            let value = (255.0 * step as f32 / 9.0).round() as u8;
            format!("{value:02X}{value:02X}{value:02X}")
        })
        .collect();
    let standard: Vec<String> = STANDARD_COLORS.iter().map(|c| c.to_string()).collect();
    let preview = |color: &str| {
        div()
            .w(px(56.0))
            .h(px(20.0))
            .bg(gpui::rgb(u32::from_str_radix(color, 16).unwrap_or(0)))
    };
    div()
        .id("color-dialog")
        .w(px(320.0))
        .flex()
        .flex_col()
        .gap_3()
        // A drag keeps going over the whole dialog, the point outside the
        // wheel taken at its edge.
        .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _, cx| {
            let Some(Dialog::Color(dialog)) = view.characters.dialog.as_mut() else {
                return;
            };
            let Some(drag) = dialog.drag else {
                return;
            };
            if !event.dragging() {
                dialog.drag = None;
                return;
            }
            edit(view, |dialog| dialog.drag_to(drag, event.position));
            cx.notify();
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|view, _: &MouseUpEvent, _, cx| {
                if let Some(Dialog::Color(dialog)) = view.characters.dialog.as_mut() {
                    dialog.drag = None;
                }
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|view, _: &MouseUpEvent, _, _| {
                if let Some(Dialog::Color(dialog)) = view.characters.dialog.as_mut() {
                    dialog.drag = None;
                }
            }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(wheel(dialog, cx))
                .child(slider(dialog, cx))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(caption(i18n::text("chars.color.new")))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .border_1()
                                .border_color(theme::border())
                                .child(preview(&new))
                                .child(preview(&current)),
                        )
                        .child(caption(i18n::text("chars.color.current"))),
                ),
        )
        .child(swatches("grey", greys, &new, cx))
        .child(swatches("standard", standard, &new, cx))
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(caption(i18n::text("chars.color.code")))
                .child(
                    div()
                        .min_w(px(90.0))
                        .px_1()
                        .border_1()
                        .border_color(theme::accent())
                        .bg(theme::canvas())
                        .child(if typed.is_empty() {
                            format!("#{new}")
                        } else {
                            format!("{typed}▏")
                        }),
                ),
        )
        .into_any_element()
}

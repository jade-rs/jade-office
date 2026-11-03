//! 図の形式 › トリミング on a slide, as PowerPoint for Mac does it: the
//! selected picture shows the whole picture faintly round the part kept,
//! and black crop handles on that part's sides and corners; dragging one
//! cuts into the picture (or back out of what was cut) without moving it.
//! The crop is written when the handle is let go, one Undo.

use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent};
use jade_office_model::deck::{ShapeRect, SourceRect};
use jade_office_model::word::handles::{crop_drag, Handle, ImageCrop};

use super::DeckView;
use crate::theme;

/// A crop handle held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CropDrag {
    pub handle: Handle,
    pub from: (f64, f64),
    pub to: (f64, f64),
    /// The kept part's box and the crop when the drag started.
    pub rect: (f64, f64, f64, f64),
    pub crop: ImageCrop,
}

impl CropDrag {
    /// The crop and the kept part's box where the handle is now.
    pub fn result(&self) -> (ImageCrop, (f64, f64, f64, f64)) {
        crop_drag(
            self.rect,
            self.crop,
            self.handle,
            (self.to.0 - self.from.0, self.to.1 - self.from.1),
        )
    }
}

fn fractions(crop: SourceRect) -> ImageCrop {
    let f = |v: i32| f64::from(v) / 100_000.0;
    ImageCrop {
        left: f(crop.left),
        top: f(crop.top),
        right: f(crop.right),
        bottom: f(crop.bottom),
    }
}

fn thousandths(crop: ImageCrop) -> SourceRect {
    let t = |v: f64| (v * 100_000.0).round() as i32;
    SourceRect {
        left: t(crop.left),
        top: t(crop.top),
        right: t(crop.right),
        bottom: t(crop.bottom),
    }
}

/// The bar a crop handle is drawn as, centred on `(x, y)`, in window px.
fn bar(handle: Handle) -> (f32, f32) {
    const LONG: f32 = 14.0;
    const THICK: f32 = 4.0;
    match handle {
        Handle::Top | Handle::Bottom => (LONG, THICK),
        Handle::Left | Handle::Right => (THICK, LONG),
        _ => (THICK * 2.5, THICK * 2.5),
    }
}

impl DeckView {
    /// The picture in トリミング, while it is the one selected.
    pub(super) fn cropping_picture(&self) -> Option<u32> {
        let id = self.pictures.cropping?;
        (self.selected_picture() == Some(id)).then_some(id)
    }

    /// トリミング on or off for the selected picture.
    pub(super) fn toggle_crop_handles(&mut self, cx: &mut Context<Self>) {
        self.pictures.popup = None;
        self.pictures.crop_drag = None;
        self.pictures.cropping = match (self.cropping_picture(), self.selected_picture()) {
            (Some(_), _) => None,
            (None, picture) => picture,
        };
        cx.notify();
    }

    /// The whole picture's box and the kept part's, in slide points.
    fn crop_boxes(&self, id: u32) -> Option<(ImageCrop, (f64, f64, f64, f64))> {
        let rect = ShapeRect::of(self.shape(id)?.transform.as_ref()?);
        let crop = fractions(self.deck.picture_crop(self.current, id).unwrap_or_default());
        Some((crop, (rect.x, rect.y, rect.width, rect.height)))
    }

    fn start_crop_drag(&mut self, handle: Handle, event: &MouseDownEvent, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let Some(id) = self.cropping_picture() else {
            return;
        };
        let (Some(from), Some((crop, rect))) =
            (self.slide_point(event.position), self.crop_boxes(id))
        else {
            return;
        };
        self.pictures.crop_drag = Some(CropDrag {
            handle,
            from,
            to: from,
            rect,
            crop,
        });
    }

    /// A drag of a crop handle reaching `(x, y)`: `true` when one is held.
    pub(super) fn crop_drag_to(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.pictures.crop_drag.as_mut() else {
            return false;
        };
        drag.to = (x, y);
        cx.notify();
        true
    }

    /// The crop handle let go: the crop written. `true` when one was held.
    pub(super) fn finish_crop_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.pictures.crop_drag.take() else {
            return false;
        };
        if drag.to == drag.from {
            cx.notify();
            return true;
        }
        let (crop, _) = drag.result();
        self.picture_command(
            move |view, slide, id| view.deck.crop_picture(slide, id, thousandths(crop)),
            cx,
        );
        true
    }

    /// トリミング's overlay on the selected picture: the whole picture's
    /// outline, the kept part and its crop handles. `None` when no picture
    /// is being cropped.
    pub(super) fn crop_overlay(
        &self,
        scale: f32,
        cx: &mut Context<Self>,
    ) -> Option<Vec<AnyElement>> {
        let id = self.cropping_picture()?;
        let (crop, rect) = match self.pictures.crop_drag {
            Some(drag) => drag.result(),
            None => self.crop_boxes(id)?,
        };
        let (x, y, width, height) = rect;
        let full_width = width / (1.0 - crop.left - crop.right).max(0.001);
        let full_height = height / (1.0 - crop.top - crop.bottom).max(0.001);
        let full = (
            x - crop.left * full_width,
            y - crop.top * full_height,
            full_width,
            full_height,
        );
        let at = |v: f64| v as f32 * scale;
        let mut out = vec![
            div()
                .absolute()
                .left(px(at(full.0)))
                .top(px(at(full.1)))
                .w(px(at(full.2)))
                .h(px(at(full.3)))
                .bg(gpui::rgba(0x00000022))
                .border_1()
                .border_color(gpui::rgba(0x00000066))
                .into_any_element(),
            div()
                .absolute()
                .left(px(at(x)))
                .top(px(at(y)))
                .w(px(at(width)))
                .h(px(at(height)))
                .border_1()
                .border_color(theme::accent())
                .into_any_element(),
        ];
        for (index, handle) in Handle::ALL.into_iter().enumerate() {
            let (fx, fy) = handle.anchor();
            let (w, h) = bar(handle);
            let cx_ = at(x + width * fx);
            let cy_ = at(y + height * fy);
            out.push(
                div()
                    .id(("crop-handle", index))
                    .absolute()
                    .left(px(cx_ - w / 2.0))
                    .top(px(cy_ - h / 2.0))
                    .w(px(w))
                    .h(px(h))
                    .bg(gpui::black())
                    .border_1()
                    .border_color(gpui::white())
                    .cursor(match handle {
                        Handle::Left | Handle::Right => gpui::CursorStyle::ResizeLeftRight,
                        Handle::Top | Handle::Bottom => gpui::CursorStyle::ResizeUpDown,
                        Handle::TopLeft | Handle::BottomRight => {
                            gpui::CursorStyle::ResizeUpLeftDownRight
                        }
                        _ => gpui::CursorStyle::ResizeUpRightDownLeft,
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                            view.start_crop_drag(handle, event, cx)
                        }),
                    )
                    .into_any_element(),
            );
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_handle_dragged_in_cuts_and_back_out_uncuts() {
        let drag = CropDrag {
            handle: Handle::Right,
            from: (200.0, 50.0),
            to: (150.0, 50.0),
            rect: (100.0, 0.0, 100.0, 100.0),
            crop: fractions(SourceRect::default()),
        };
        let (crop, rect) = drag.result();
        assert_eq!(thousandths(crop).right, 50_000);
        assert_eq!(rect, (100.0, 0.0, 50.0, 100.0));
        let back = CropDrag {
            handle: Handle::Right,
            from: (150.0, 50.0),
            to: (200.0, 50.0),
            rect,
            crop,
        };
        assert_eq!(thousandths(back.result().0), SourceRect::default());
    }
}

#[cfg(test)]
mod view_tests {
    use std::path::PathBuf;

    use gpui::{Entity, Focusable};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};

    use super::*;

    fn open(cx: &mut gpui::TestAppContext) -> (Entity<DeckView>, &mut gpui::VisualTestContext) {
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/public/fixtures/sample-deck.pptx"
        ));
        let (view, cx) = cx.add_window_view(|_window, cx| DeckView::open(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    #[gpui::test]
    fn a_crop_handle_cuts_the_picture_in_one_undo(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        let mut png = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(RgbImage::from_pixel(80, 40, Rgb([10, 120, 10])))
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        view.update(cx, |view, cx| view.insert_picture(png.get_ref(), cx));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            let id = view.selected_picture().expect("the picture is selected");
            view.toggle_crop_handles(cx);
            assert_eq!(view.cropping_picture(), Some(id));
            let (crop, rect) = view.crop_boxes(id).unwrap();
            // The right handle dragged a quarter of the way in.
            let right = (rect.0 + rect.2, rect.1 + rect.3 / 2.0);
            view.pictures.crop_drag = Some(CropDrag {
                handle: Handle::Right,
                from: right,
                to: (right.0 - rect.2 / 4.0, right.1),
                rect,
                crop,
            });
            assert!(view.crop_overlay(1.0, cx).is_some());
            assert!(view.finish_crop_drag(cx));
        });
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            let id = view.selected_picture().unwrap();
            let crop = view.deck.picture_crop(view.current, id).unwrap();
            assert!((crop.right - 25_000).abs() <= 1, "{crop:?}");
            let (_, rect) = view.crop_boxes(id).unwrap();
            assert!((rect.2 - rect.3 * 1.5).abs() < 1.0, "{rect:?}");
            view.toggle_crop_handles(cx);
            assert!(view.cropping_picture().is_none());
            assert!(view.deck.undo().unwrap());
            assert_eq!(
                view.deck.picture_crop(view.current, id).unwrap(),
                SourceRect::default()
            );
        });
    }
}

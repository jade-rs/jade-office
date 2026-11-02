//! Animations, transitions and zooms in the browser's slide show.
//!
//! The same playback the native show runs (jade-office-model's
//! `show_playback`), with the browser's clock: clicks play effects before
//! they change slides, a slide's 画面切り替え runs into it, triggers and
//! zooms answer clicks on them, and 自動 advances on time.

use jade_web::deck::show_playback::{ShowPicture, ShowPlayback, ShowStep};

use super::*;

/// The show's playback state and the frame it last painted.
#[derive(Default)]
pub(super) struct WebShow {
    playback: ShowPlayback,
    shown: Option<Arc<RenderImage>>,
    /// Where the slide was drawn, for clicks on triggers and zooms.
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

/// Milliseconds on a clock that only goes forward.
fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_secs_f64()
            * 1000.0
    }
}

impl DocView {
    /// The show starts afresh.
    pub(super) fn reset_show(&mut self) {
        let old = std::mem::take(&mut self.show);
        self.retired.extend(old.shown);
    }

    /// A click or key in the show: an effect first, then the slide.
    /// Returns the slide to go to, `None` when the effect took it or the
    /// show is over.
    pub(super) fn show_step(&mut self, forward: bool, cx: &mut Context<Self>) -> Option<usize> {
        let Document::Deck { deck, current, .. } = &self.document else {
            return None;
        };
        match self.show.playback.step(deck, *current, forward, now_ms()) {
            ShowStep::Handled => {
                cx.notify();
                None
            }
            ShowStep::Goto(slide) => Some(slide),
            ShowStep::Slide => match deck.next_shown(*current, forward) {
                Some(next) => Some(next),
                None => {
                    if forward {
                        self.slideshow = false;
                        cx.notify();
                    }
                    None
                }
            },
        }
    }

    /// Goes to slide `next` in the show, running its transition.
    pub(super) fn show_go(&mut self, next: usize, forward: bool, cx: &mut Context<Self>) {
        if let Document::Deck {
            deck,
            current,
            slide,
            ..
        } = &mut self.document
        {
            let width = slide.as_ref().map_or(1280, |(_, width, _, _)| *width);
            self.show
                .playback
                .entered(deck, *current, next, forward, width, now_ms());
        }
        self.go_to_slide(next, cx);
    }

    /// A left click in the show: a trigger or a zoom under it, else the
    /// next step.
    pub(super) fn show_click(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let point = self.show.bounds.get().and_then(|bounds| {
            let Document::Deck { deck, .. } = &self.document else {
                return None;
            };
            let scale = f64::from(f32::from(bounds.size.width)) / deck.slide_size().0.max(1.0);
            let x = f64::from(f32::from(event.position.x - bounds.origin.x)) / scale;
            let y = f64::from(f32::from(event.position.y - bounds.origin.y)) / scale;
            (x >= 0.0 && y >= 0.0).then_some((x, y))
        });
        let outcome = match (&mut self.document, point) {
            (Document::Deck { deck, current, .. }, Some(point)) => {
                self.show.playback.click_at(deck, *current, point, now_ms())
            }
            _ => ShowStep::Slide,
        };
        match outcome {
            ShowStep::Handled => cx.notify(),
            ShowStep::Goto(slide) => self.show_go(slide, true, cx),
            ShowStep::Slide => self.step_show(true, cx),
        }
    }

    /// The animated frame to show instead of the plain slide, if any;
    /// asks for the next frame while anything moves, and advances when
    /// 自動's time is up.
    pub(super) fn show_picture(
        &mut self,
        device_width: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let now = now_ms();
        let due = match &self.document {
            Document::Deck { deck, current, .. } => self.show.playback.due(deck, *current, now),
            _ => false,
        };
        if due {
            self.step_show(true, cx);
        }
        let Document::Deck { deck, current, .. } = &mut self.document else {
            return None;
        };
        let (picture, ticking) =
            self.show
                .playback
                .picture(deck, *current, device_width.max(1), now);
        if ticking {
            window.request_animation_frame();
        }
        match picture {
            ShowPicture::Plain => {
                self.retired.extend(self.show.shown.take());
                None
            }
            ShowPicture::Same => self.show.shown.clone(),
            ShowPicture::Frame {
                pixels,
                width,
                height,
            } => {
                let buffer = image::ImageBuffer::from_raw(width, height, pixels)?;
                let image = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
                self.retired.extend(self.show.shown.replace(image.clone()));
                Some(image)
            }
        }
    }

    /// Records where the show's slide is drawn.
    pub(super) fn show_bounds_recorder(&self) -> impl IntoElement {
        let store = Rc::clone(&self.show.bounds);
        canvas(
            move |bounds, _window, _cx| store.set(Some(bounds)),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}

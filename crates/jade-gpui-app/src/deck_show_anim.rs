//! Animations, transitions and zooms in the slide show.
//!
//! A thin layer over jade-office-model's [`show_playback`]: it owns the
//! clock and the texture of the animated frame, and asks the window for
//! the next frame while anything moves. The show loop calls it through
//! small hooks: [`ShowPlayback::step`] on a click or key,
//! [`ShowPlayback::click_at`] on a click at a point,
//! [`ShowPlayback::entered`] when the slide changes and
//! [`ShowPlayback::image`] while rendering.
//!
//! [`show_playback`]: jade_office_model::deck::show_playback

use std::sync::Arc;
use std::time::Instant;

use gpui::{RenderImage, Window};
use jade_office_model::deck::show_playback::{self, ShowPicture};
use jade_office_model::deck::Deck;
use jade_office_model::skia::SkiaBackend;

pub(crate) use jade_office_model::deck::show_playback::ShowStep;

pub(crate) struct ShowPlayback {
    clock: Instant,
    inner: show_playback::ShowPlayback,
    shown: Option<Arc<RenderImage>>,
}

impl Default for ShowPlayback {
    fn default() -> Self {
        Self {
            clock: Instant::now(),
            inner: Default::default(),
            shown: None,
        }
    }
}

impl ShowPlayback {
    fn now(&self) -> f64 {
        self.clock.elapsed().as_secs_f64() * 1000.0
    }

    /// A click or key on `current`: plays the next (or undoes the last)
    /// animation, or says where the show goes.
    pub(crate) fn step(
        &mut self,
        deck: &mut Deck<SkiaBackend>,
        current: usize,
        forward: bool,
    ) -> ShowStep {
        let now = self.now();
        self.inner.step(deck, current, forward, now)
    }

    /// The show moved from `from` to `to`.
    pub(crate) fn entered(
        &mut self,
        deck: &mut Deck<SkiaBackend>,
        from: usize,
        to: usize,
        forward: bool,
        device_width: i32,
    ) {
        let now = self.now();
        let width = device_width.max(1) as u32;
        self.inner.entered(deck, from, to, forward, width, now);
    }

    /// A left click at `point` (slide px): a trigger or a zoom.
    pub(crate) fn click_at(
        &mut self,
        deck: &mut Deck<SkiaBackend>,
        current: usize,
        point: (f64, f64),
    ) -> ShowStep {
        let now = self.now();
        self.inner.click_at(deck, current, point, now)
    }

    /// When the slide's 自動 time has passed and nothing is playing.
    pub(crate) fn due(&self, deck: &Deck<SkiaBackend>, current: usize) -> bool {
        self.inner.due(deck, current, self.now())
    }

    /// The picture to show for `current` instead of the plain slide, or
    /// `None` when the plain slide is what shows.
    pub(crate) fn image(
        &mut self,
        deck: &mut Deck<SkiaBackend>,
        current: usize,
        device_width: i32,
        window: &mut Window,
        retired: &mut Vec<Arc<RenderImage>>,
    ) -> Option<Arc<RenderImage>> {
        let now = self.now();
        let width = device_width.max(1) as u32;
        let (picture, ticking) = self.inner.picture(deck, current, width, now);
        if ticking {
            window.request_animation_frame();
        }
        match picture {
            ShowPicture::Plain => {
                retired.extend(self.shown.take());
                None
            }
            ShowPicture::Same => self.shown.clone(),
            ShowPicture::Frame {
                pixels,
                width,
                height,
            } => {
                let buffer = image::ImageBuffer::from_raw(width, height, pixels)?;
                let image = Arc::new(RenderImage::new(vec![image::Frame::new(buffer)]));
                retired.extend(self.shown.replace(image.clone()));
                Some(image)
            }
        }
    }
}

/// Where `position` (window px) falls on the slide shown centred in
/// `viewport`, in slide px.
pub(crate) fn slide_point(
    deck: &Deck<SkiaBackend>,
    viewport: (f32, f32),
    position: (f32, f32),
) -> Option<(f64, f64)> {
    let (slide_width, slide_height) = deck.slide_size();
    let scale = (f64::from(viewport.0) / slide_width.max(1.0))
        .min(f64::from(viewport.1) / slide_height.max(1.0))
        .max(0.05);
    let left = (f64::from(viewport.0) - slide_width * scale) / 2.0;
    let top = (f64::from(viewport.1) - slide_height * scale) / 2.0;
    let x = (f64::from(position.0) - left) / scale;
    let y = (f64::from(position.1) - top) / scale;
    (x >= 0.0 && y >= 0.0 && x <= slide_width && y <= slide_height).then_some((x, y))
}

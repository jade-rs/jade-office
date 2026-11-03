//! Animations, transitions and zooms in the slide show, for both shells.
//!
//! A show keeps one [`ShowPlayback`]: it turns clicks into animation steps
//! before they become slide changes, runs the 画面切り替え into a slide
//! (Morph included) and the zoom into and back from a zoom's slides, and
//! says what to show — the plain slide, or BGRA pixels of an animated
//! frame. The shell owns the clock (`now`, ms) and the texture; it calls
//! [`ShowPlayback::step`] on a click or key, [`ShowPlayback::click_at`] on a
//! click at a point, [`ShowPlayback::entered`] when the slide changes and
//! [`ShowPlayback::picture`] while rendering.

use super::animation::{
    compose_transition, Advance, MorphOption, ShowTransition, SlidePlayer, TransitionEffect,
    TransitionFrame, TransitionSchema, ZoomFrame,
};
use super::Deck;
use crate::Backend;

/// What a click or key did, for the show loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowStep {
    /// Taken here: an animation started, finished or was undone.
    Handled,
    /// Go to this slide (a zoom's slide, or back to the zoom).
    Goto(usize),
    /// Nothing to play: the show moves on as it would.
    Slide,
}

/// What to show for the current slide.
#[derive(Debug, Clone, PartialEq)]
pub enum ShowPicture {
    /// The plain slide: nothing animates on it.
    Plain,
    /// The picture given last time still stands.
    Same,
    /// A new frame, BGRA premultiplied.
    Frame {
        pixels: Vec<u8>,
        width: u32,
        height: u32,
    },
}

/// A transition being run into `to`.
#[derive(Debug, Clone)]
struct Running {
    to: usize,
    from: usize,
    started: f64,
    duration: f64,
    effect: TransitionEffect,
    morph: Option<MorphOption>,
    /// The outgoing slide as it was left, BGRA.
    from_pixels: Vec<u8>,
    width: u32,
    height: u32,
}

/// A zoom being followed: where to come back to and when.
#[derive(Debug, Clone)]
struct ZoomTrip {
    home: usize,
    last: usize,
    return_to_parent: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ShowPlayback {
    slide: Option<usize>,
    player: Option<SlidePlayer>,
    /// When the slide came up, ms.
    entered_at: f64,
    /// Bumped on every click so a still frame repaints once.
    generation: u64,
    running: Option<Running>,
    trip: Option<ZoomTrip>,
    /// A zoom clicked, whose transition plays into its slide.
    zooming: Option<ZoomFrame>,
    /// Coming back to a zoom's slide, shown as it was left.
    back_to_zoom: bool,
    /// What the last frame given showed: slide, generation, width.
    shown: Option<(usize, u64, u32)>,
}

impl ShowPlayback {
    /// The show is on `slide`: its animations set up from the start, or
    /// played through when the show came back to it.
    fn ensure<B: Backend>(&mut self, deck: &Deck<B>, slide: usize, finished: bool, now: f64) {
        if self.slide == Some(slide) {
            return;
        }
        let mut player = deck.player(slide);
        if finished {
            player.finish_all(now);
        } else {
            player.start(now);
        }
        self.player = Some(player);
        self.slide = Some(slide);
        self.entered_at = now;
        self.generation += 1;
    }

    /// A click or key on `current`: plays the next (or undoes the last)
    /// animation, or says where the show goes.
    pub fn step<B: Backend>(
        &mut self,
        deck: &Deck<B>,
        current: usize,
        forward: bool,
        now: f64,
    ) -> ShowStep {
        // A click during a transition lands it.
        if let Some(running) = self.running.take() {
            if running.to == current {
                self.generation += 1;
                return ShowStep::Handled;
            }
        }
        self.ensure(deck, current, false, now);
        let Some(player) = self.player.as_mut() else {
            return ShowStep::Slide;
        };
        let advance = if forward {
            player.next(now)
        } else {
            player.previous()
        };
        if advance == Advance::Animated {
            self.generation += 1;
            return ShowStep::Handled;
        }
        // Past a zoom's last slide: back to the zoom (ズームに戻る).
        if let Some(trip) = self.trip.clone() {
            if forward && current == trip.last && trip.return_to_parent {
                self.trip = None;
                self.back_to_zoom = true;
                return ShowStep::Goto(trip.home);
            }
            if current == trip.last && !trip.return_to_parent {
                self.trip = None;
            }
        }
        ShowStep::Slide
    }

    /// The show moved from `from` to `to`. Forward moves run `to`'s
    /// transition (or the zoom's); going back shows `to` as it was left.
    pub fn entered<B: Backend>(
        &mut self,
        deck: &mut Deck<B>,
        from: usize,
        to: usize,
        forward: bool,
        width: u32,
        now: f64,
    ) {
        let back_to_zoom = std::mem::take(&mut self.back_to_zoom);
        let zoom = self.zooming.take();
        let outgoing = self.capture(deck, from, width, now);
        self.slide = None;
        self.ensure(deck, to, !forward || back_to_zoom, now);
        let transition = match &zoom {
            Some(zoom) => Some(ShowTransition {
                effect: TransitionEffect::new(TransitionSchema::Base, "zoom"),
                duration_ms: zoom.transition_ms,
                morph: None,
            }),
            None if back_to_zoom => Some(ShowTransition {
                effect: TransitionEffect::new(TransitionSchema::Base, "zoom")
                    .with_attr("dir", "out"),
                duration_ms: 1000,
                morph: None,
            }),
            None if forward => deck.show_transition(to),
            None => None,
        };
        self.running = match (transition, outgoing) {
            (Some(transition), Some((pixels, width, height))) if transition.duration_ms > 0 => {
                Some(Running {
                    to,
                    from,
                    started: now,
                    duration: f64::from(transition.duration_ms),
                    effect: transition.effect,
                    morph: transition.morph,
                    from_pixels: pixels,
                    width,
                    height,
                })
            }
            _ => None,
        };
        // The new slide's own animations wait for its transition.
        if let (Some(running), Some(player)) = (&self.running, self.player.as_mut()) {
            if forward && !back_to_zoom {
                player.start(running.started + running.duration);
            }
        }
    }

    /// A left click at `point` (slide px) on `current`: a trigger shape
    /// plays its sequence; a zoom goes to its slide.
    pub fn click_at<B: Backend>(
        &mut self,
        deck: &mut Deck<B>,
        current: usize,
        point: (f64, f64),
        now: f64,
    ) -> ShowStep {
        self.ensure(deck, current, false, now);
        if let Some(id) = deck.shape_at(current, point.0, point.1) {
            if let Some(player) = self.player.as_mut() {
                if player.is_trigger(id) && player.click_shape(id, now) {
                    self.generation += 1;
                    return ShowStep::Handled;
                }
            }
        }
        if let Some(zoom) = deck.zoom_at(current, point.0, point.1) {
            let slides = deck.zoom_slides(&zoom);
            if let (Some(&first), Some(&last)) = (slides.first(), slides.last()) {
                self.trip = Some(ZoomTrip {
                    home: current,
                    last,
                    return_to_parent: zoom.return_to_parent,
                });
                self.zooming = Some(zoom);
                return ShowStep::Goto(first);
            }
        }
        ShowStep::Slide
    }

    /// A media bookmark reached on `current`: its trigger's sequence plays.
    pub fn bookmark_reached(&mut self, shape_id: u32, bookmark: &str, now: f64) {
        if let Some(player) = self.player.as_mut() {
            player.bookmark_reached(shape_id, bookmark, now);
            self.generation += 1;
        }
    }

    /// Whether the slide's 自動 time has passed with nothing playing.
    pub fn due<B: Backend>(&self, deck: &Deck<B>, current: usize, now: f64) -> bool {
        if self.slide != Some(current) || self.running.is_some() {
            return false;
        }
        let Some(after) = deck
            .transition(current)
            .and_then(|transition| transition.advance_after_ms)
        else {
            return false;
        };
        let settled = self
            .player
            .as_ref()
            .map_or(Some(self.entered_at), SlidePlayer::settled_at);
        settled.is_some_and(|settled| now >= settled.max(self.entered_at) + f64::from(after))
    }

    /// What to show for `current` at `now`, `width` device px wide, and
    /// whether the shell should paint again next frame (something moves,
    /// or 自動 is waiting).
    pub fn picture<B: Backend>(
        &mut self,
        deck: &mut Deck<B>,
        current: usize,
        width: u32,
        now: f64,
    ) -> (ShowPicture, bool) {
        self.ensure(deck, current, false, now);
        let waits = deck
            .transition(current)
            .is_some_and(|transition| transition.advance_after_ms.is_some());
        if let Some(running) = &self.running {
            if running.to == current {
                let progress = (now - running.started) / running.duration.max(1.0);
                if progress < 1.0 {
                    self.shown = None;
                    let picture = self.transition_frame(deck, progress).map_or(
                        ShowPicture::Plain,
                        |(pixels, width, height)| ShowPicture::Frame {
                            pixels,
                            width,
                            height,
                        },
                    );
                    return (picture, true);
                }
            }
            self.running = None;
            self.generation += 1;
        }
        let Some(player) = self.player.as_ref() else {
            return (ShowPicture::Plain, waits);
        };
        let animating = player.is_animating(now);
        let frame = deck.frame(current, player, now);
        if frame.is_empty() {
            self.shown = None;
            return (ShowPicture::Plain, animating || waits);
        }
        let key = (current, self.generation, width);
        if !animating && self.shown == Some(key) {
            return (ShowPicture::Same, waits);
        }
        let Some(painter) = deck.render_frame(current, width.max(1), &frame) else {
            return (ShowPicture::Plain, animating || waits);
        };
        let Some((pixels, width_px, height)) = B::bgra(&painter) else {
            return (ShowPicture::Plain, animating || waits);
        };
        self.shown = (!animating).then_some(key);
        (
            ShowPicture::Frame {
                pixels,
                width: width_px,
                height,
            },
            animating || waits,
        )
    }

    fn transition_frame<B: Backend>(
        &self,
        deck: &mut Deck<B>,
        progress: f64,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let running = self.running.as_ref()?;
        if let Some(option) = running.morph {
            let painter =
                deck.render_morph(running.from, running.to, running.width, option, progress)?;
            return B::bgra(&painter);
        }
        let player = self.player.as_ref()?;
        // The incoming slide as it first shows: entrances still hidden.
        let frame = deck.frame(running.to, player, running.started);
        let painter = deck.render_frame(running.to, running.width, &frame)?;
        let (to, width, height) = B::bgra(&painter)?;
        if (width, height) != (running.width, running.height) {
            return Some((to, width, height));
        }
        let pixels = compose_transition(
            TransitionFrame {
                from: &running.from_pixels,
                to: &to,
                width,
                height,
            },
            &running.effect,
            progress,
        );
        Some((pixels, width, height))
    }

    /// `slide` as it stands now, BGRA.
    fn capture<B: Backend>(
        &self,
        deck: &mut Deck<B>,
        slide: usize,
        width: u32,
        now: f64,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let width = width.max(1);
        let painter = match (&self.player, self.slide) {
            (Some(player), Some(shown)) if shown == slide => {
                let frame = deck.frame(slide, player, now);
                deck.render_frame(slide, width, &frame)?
            }
            _ => deck.render_slide(slide, width)?,
        };
        B::bgra(&painter)
    }
}

#[cfg(all(test, feature = "tiny-skia"))]
mod tests {
    use super::*;
    use crate::deck::animation::{AnimationEdit, AnimationTarget, EffectParams, PresetClass};
    use crate::deck::animation::{TransitionEdit, TRANSITIONS};
    use crate::test_support::{faces, fixture};
    use crate::tiny::TinySkiaBackend;

    #[test]
    fn a_show_plays_entrances_on_clicks_then_runs_the_transition() {
        let mut deck = Deck::open(
            &fixture("sample-deck.pptx"),
            TinySkiaBackend::new(&faces()).unwrap(),
        )
        .unwrap();
        let shape = deck.selectable_shapes(0)[0];
        deck.edit_animation(
            0,
            &AnimationEdit::Add {
                class: PresetClass::Entrance,
                preset_id: 10,
                params: EffectParams::default(),
                targets: vec![AnimationTarget::shape(shape)],
                start: None,
                after: None,
            },
        )
        .unwrap();
        let fade = TRANSITIONS.iter().find(|t| t.key == "fade").unwrap();
        let effect = crate::deck::animation::preset_effect(fade, None);
        deck.set_transition(&[1], &TransitionEdit::Effect(Some(effect)))
            .unwrap();

        let mut show = ShowPlayback::default();
        // Before the click the shape is hidden: an animated frame shows.
        let (picture, _) = show.picture(&mut deck, 0, 160, 0.0);
        assert!(matches!(picture, ShowPicture::Frame { .. }));
        assert_eq!(show.picture(&mut deck, 0, 160, 10.0).0, ShowPicture::Same);
        // The click plays the fade; half way it is still moving.
        assert_eq!(show.step(&deck, 0, true, 100.0), ShowStep::Handled);
        let (picture, ticking) = show.picture(&mut deck, 0, 160, 350.0);
        assert!(matches!(picture, ShowPicture::Frame { .. }) && ticking);
        // Done: the next click goes on to slide 2, whose fade runs.
        let (_, ticking) = show.picture(&mut deck, 0, 160, 2000.0);
        assert!(!ticking);
        assert_eq!(show.step(&deck, 0, true, 2100.0), ShowStep::Slide);
        show.entered(&mut deck, 0, 1, true, 160, 2100.0);
        let (picture, ticking) = show.picture(&mut deck, 1, 160, 2300.0);
        assert!(matches!(picture, ShowPicture::Frame { .. }) && ticking);
        // A click lands the transition at once.
        assert_eq!(show.step(&deck, 1, true, 2310.0), ShowStep::Handled);
        assert_eq!(
            show.picture(&mut deck, 1, 160, 2320.0).0,
            ShowPicture::Plain
        );
    }
}

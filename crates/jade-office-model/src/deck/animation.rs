//! The アニメーション and 画面切り替え tabs, ズーム, and the slide show's
//! playback, as both shells use them.
//!
//! Edits go through the writer (`p:timing`, `p:transition`, zoom frames,
//! each one step of Undo); playback reads the same markup back into a
//! [`SlidePlayer`] and paints frames with jade-pptx-render's animation
//! renderer.

use jade_painter::{ImageRect, ImageSource, Painter};
use jade_pptx_layout::PlacedShape;
use jade_pptx_reader::{ShapeItem, TextBody};
use jade_pptx_render::animation::evaluate::TargetBase;
use jade_pptx_render::animation::morph::render_morph_frame;
use jade_pptx_render::animation::{render_slide_frame, target_bounds};
use jade_pptx_render::{DrawImage, RenderOptions};

pub use jade_pptx_reader::animation::{
    AfterEffect, AnimationEffect, AnimationTarget, EffectClass, EffectStart, IterateUnit, Repeat,
    SlideAnimation, SlideTransition, TargetPart, TransitionEffect, TransitionSchema, Trigger,
    ZoomFrame, ZoomKind,
};
pub use jade_pptx_render::animation::{
    compose_transition, Advance, MorphOption, SlideFrame, SlidePlayer, SlideText, TargetKey,
    TransitionFrame,
};
pub use jade_pptx_writer::animation::transition::{
    preset_effect, transition_preset, SoundChoice, TransitionEdit, TransitionPreset, TRANSITIONS,
};
pub use jade_pptx_writer::animation::zoom::{Section, ZoomEntry, ZoomOptions, ZoomTarget};
pub use jade_pptx_writer::animation::{
    preset, AnimationEdit, ChartSequence, EffectParams, EffectPreset, PaneEffect, PresetClass,
    TextSequence, PRESETS,
};

use super::{Deck, ShapeRect, EMU_PER_PX};
use crate::Backend;

/// The 画面切り替え a show runs into slide `to`.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowTransition {
    pub effect: TransitionEffect,
    pub duration_ms: u32,
    /// Morph's 効果のオプション, when it is Morph.
    pub morph: Option<MorphOption>,
}

impl<B: Backend> Deck<B> {
    /// Slide `slide`'s timing, transition and zooms as written.
    pub fn slide_animation(&self, slide: usize) -> SlideAnimation {
        jade_pptx_writer::animation::slide_animation(self.bytes(), slide).unwrap_or_default()
    }

    /// The アニメーション ウィンドウ's list: the main sequence, then each
    /// trigger's effects.
    pub fn animation_pane(&self, slide: usize) -> Vec<PaneEffect> {
        jade_pptx_writer::animation::pane_effects(&self.slide_animation(slide).timing)
    }

    /// One edit on the アニメーション tab, as one step of Undo.
    pub fn edit_animation(&mut self, slide: usize, edit: &AnimationEdit) -> Result<bool, String> {
        let result = self.session.edit_animation(slide, edit);
        self.after(result)
    }

    /// One edit on the 画面切り替え tab for `slides` (すべてに適用 passes
    /// every slide), as one step of Undo.
    pub fn set_transition(
        &mut self,
        slides: &[usize],
        edit: &TransitionEdit,
    ) -> Result<bool, String> {
        let result = self.session.set_transition(slides, edit);
        self.after(result)
    }

    /// Slide `slide`'s transition, `None` when it has none.
    pub fn transition(&self, slide: usize) -> Option<SlideTransition> {
        self.slide_animation(slide).transition
    }

    /// What the show runs when it moves on to `slide`.
    pub fn show_transition(&self, slide: usize) -> Option<ShowTransition> {
        let transition = self.transition(slide)?;
        let effect = transition.effect.clone()?;
        let morph = (effect.name == "morph").then(|| MorphOption::parse(effect.attr("option")));
        Some(ShowTransition {
            duration_ms: transition.effective_duration_ms(),
            effect,
            morph,
        })
    }

    /// Each shape's text, paragraph by paragraph, for by-paragraph, by-word
    /// and by-letter playback.
    pub fn slide_text(&self, slide: usize) -> SlideText {
        let mut text = SlideText::new();
        if let Some(slide) = self.deck.slides.get(slide) {
            collect_text(&slide.content.shapes, &mut text);
        }
        text
    }

    /// A player for slide `slide`'s animations.
    pub fn player(&self, slide: usize) -> SlidePlayer {
        SlidePlayer::new(&self.slide_animation(slide).timing, self.slide_text(slide))
    }

    /// The state of slide `slide`'s targets at `now` on `player`'s clock.
    pub fn frame(&mut self, slide: usize, player: &SlidePlayer, now: f64) -> SlideFrame {
        let Some(layout) = self.layout(slide) else {
            return SlideFrame::default();
        };
        let text = self.slide_text(slide);
        player.frame(now, &|key: &TargetKey| -> TargetBase {
            target_bounds(&layout, &text, key)
        })
    }

    /// Paints slide `slide` `width` device px wide as `frame` has it.
    pub fn render_frame(
        &mut self,
        slide: usize,
        width: u32,
        frame: &SlideFrame,
    ) -> Option<B::Painter> {
        let layout = self.layout(slide)?;
        let text = self.slide_text(slide);
        let (painter, scale) = self.surface(width)?;
        self.paint_with(&painter, scale, |painter, options, draw_image| {
            render_slide_frame(painter, &layout, options, draw_image, frame, &text).is_ok()
        })?;
        Some(painter)
    }

    /// Paints Morph `progress` of the way from slide `from` to slide `to`.
    pub fn render_morph(
        &mut self,
        from: usize,
        to: usize,
        width: u32,
        option: MorphOption,
        progress: f64,
    ) -> Option<B::Painter> {
        let (old, new) = (self.layout(from)?, self.layout(to)?);
        let (painter, scale) = self.surface(width)?;
        self.paint_with(&painter, scale, |painter, options, draw_image| {
            render_morph_frame(painter, &old, &new, options, draw_image, option, progress).is_ok()
        })?;
        Some(painter)
    }

    /// The zooms on slide `slide`.
    pub fn zooms(&self, slide: usize) -> Vec<ZoomFrame> {
        self.slide_animation(slide).zooms
    }

    /// The zoom under (`x`, `y`) slide px, the topmost.
    pub fn zoom_at(&self, slide: usize, x: f64, y: f64) -> Option<ZoomFrame> {
        self.zooms(slide).into_iter().rev().find(|zoom| {
            let rect = ShapeRect {
                x: zoom.rect.0 as f64 / EMU_PER_PX,
                y: zoom.rect.1 as f64 / EMU_PER_PX,
                width: zoom.rect.2 as f64 / EMU_PER_PX,
                height: zoom.rect.3 as f64 / EMU_PER_PX,
            };
            rect.contains(x, y)
        })
    }

    /// The index of the slide whose `p:sldId` is `id`.
    pub fn slide_index_of(&self, id: u32) -> Option<usize> {
        self.deck
            .slides
            .iter()
            .position(|slide| slide.id == Some(id))
    }

    /// Slide `index`'s `p:sldId` id.
    pub fn slide_id(&self, index: usize) -> Option<u32> {
        self.deck.slides.get(index).and_then(|slide| slide.id)
    }

    /// A chart's series and category counts, for グラフ アニメーション's
    /// by-series and by-category sequences; `None` when `shape_id` is not
    /// a chart.
    pub fn chart_size(&self, slide: usize, shape_id: u32) -> Option<(u32, u32)> {
        fn find(shapes: &[ShapeItem], id: u32) -> Option<(u32, u32)> {
            shapes.iter().find_map(|item| match item {
                ShapeItem::Chart(chart) if chart.id == Some(id) => {
                    let categories = chart
                        .series
                        .iter()
                        .map(|series| series.categories.len().max(series.values.len()))
                        .max()
                        .unwrap_or(0);
                    Some((chart.series.len() as u32, categories as u32))
                }
                ShapeItem::Group(group) => find(&group.children, id),
                _ => None,
            })
        }
        find(&self.deck.slides.get(slide)?.content.shapes, shape_id)
    }

    /// The presentation's sections.
    pub fn sections(&self) -> Vec<Section> {
        jade_pptx_writer::animation::zoom::sections(self.bytes()).unwrap_or_default()
    }

    /// The slides a zoom shows, in order: one slide, or a whole section.
    pub fn zoom_slides(&self, zoom: &ZoomFrame) -> Vec<usize> {
        if let Some(id) = zoom.slide_id {
            return self.slide_index_of(id).into_iter().collect();
        }
        let Some(section) = zoom.section_id.as_deref() else {
            return Vec::new();
        };
        self.sections()
            .into_iter()
            .find(|candidate| candidate.id == section)
            .map(|section| {
                section
                    .slide_ids
                    .iter()
                    .filter_map(|&id| self.slide_index_of(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 挿入 > ズーム: a zoom to each of `targets` on slide `slide`, laid
    /// out from (`x`, `y`) slide px at a quarter of the slide's size each
    /// (a summary zoom puts them all in one frame). Returns the new ids.
    pub fn insert_zoom(
        &mut self,
        slide: usize,
        kind: ZoomKind,
        targets: &[ZoomTarget],
        at: (f64, f64),
    ) -> Result<Vec<u32>, String> {
        let (slide_width, slide_height) = self.slide_size();
        let (width, height) = (slide_width / 4.0, slide_height / 4.0);
        let gap = width / 8.0;
        let mut entries = Vec::new();
        for (index, target) in targets.iter().enumerate() {
            let first = match target {
                ZoomTarget::Slide(id) => self.slide_index_of(*id),
                ZoomTarget::Section(section) => self
                    .sections()
                    .into_iter()
                    .find(|candidate| &candidate.id == section)
                    .and_then(|section| section.slide_ids.first().copied())
                    .and_then(|id| self.slide_index_of(id)),
            }
            .ok_or_else(|| "the zoom's slide is not in the presentation".to_string())?;
            let painter = self
                .render_slide(first, 480)
                .ok_or_else(|| "cannot paint the zoom's slide".to_string())?;
            let image =
                B::png(&painter).ok_or_else(|| "cannot encode the zoom's image".to_string())?;
            let column = (index % 3) as f64;
            let row = (index / 3) as f64;
            entries.push(ZoomEntry {
                target: target.clone(),
                image,
                rect: (
                    emu(column * (width + gap)),
                    emu(row * (height + gap)),
                    emu(width),
                    emu(height),
                ),
            });
        }
        let options = ZoomOptions::defaults(kind);
        let mut ids = Vec::new();
        if kind == ZoomKind::Summary {
            let columns = entries.len().min(3) as f64;
            let rows = entries.len().div_ceil(3) as f64;
            let frame = (
                emu(at.0),
                emu(at.1),
                emu(columns * width + (columns - 1.0).max(0.0) * gap),
                emu(rows * height + (rows - 1.0).max(0.0) * gap),
            );
            let result = self
                .session
                .insert_zoom(slide, kind, frame, &entries, options);
            if let Some(id) = result.map_err(|error| error.to_string())? {
                ids.push(id);
            }
        } else {
            for entry in entries {
                let frame = (
                    emu(at.0) + entry.rect.0,
                    emu(at.1) + entry.rect.1,
                    entry.rect.2,
                    entry.rect.3,
                );
                let result = self.session.insert_zoom(
                    slide,
                    kind,
                    frame,
                    std::slice::from_ref(&entry),
                    options,
                );
                if let Some(id) = result.map_err(|error| error.to_string())? {
                    ids.push(id);
                }
            }
        }
        if !ids.is_empty() {
            self.reload()?;
        }
        Ok(ids)
    }

    /// ズーム > ズームのオプション.
    pub fn set_zoom_options(
        &mut self,
        slide: usize,
        shape_id: u32,
        options: ZoomOptions,
    ) -> Result<bool, String> {
        let result = self.session.set_zoom_options(slide, shape_id, options);
        self.after(result)
    }

    /// A surface `width` device px wide at the slide's aspect, and the
    /// scale from slide px to it.
    fn surface(&self, width: u32) -> Option<(B::Painter, f64)> {
        let (slide_width, slide_height) = self.slide_size();
        let scale = f64::from(width.max(1)) / slide_width.max(1.0);
        let height = (slide_height * scale).round().max(1.0) as u32;
        Some((self.backend.surface(width.max(1), height)?, scale))
    }

    /// Runs `paint` scaled, with pictures drawn from the package's parts.
    fn paint_with(
        &self,
        painter: &B::Painter,
        scale: f64,
        paint: impl FnOnce(&B::Painter, &RenderOptions, &mut DrawImage<'_>) -> bool,
    ) -> Option<()> {
        painter.save();
        let scaled = painter.scale(scale, scale).is_ok();
        let parts = &self.parts;
        let mut draw_image = |shape: &PlacedShape| -> bool {
            let Some((part, bytes)) = shape.image_part.as_deref().and_then(|part| {
                parts
                    .get(part.trim_start_matches('/'))
                    .or_else(|| parts.get(part))
                    .map(|bytes| (part, bytes))
            }) else {
                return false;
            };
            let Some((source, destination)) = shape.image_rectangles(1.0, 1.0) else {
                return false;
            };
            painter
                .draw_image(
                    ImageSource { key: part, bytes },
                    Some([source.x, source.y, source.width, source.height]),
                    ImageRect {
                        x: destination.x,
                        y: destination.y,
                        width: destination.width,
                        height: destination.height,
                    },
                )
                .unwrap_or(false)
        };
        let painted = scaled && paint(painter, &self.options, &mut draw_image);
        painter.restore();
        painted.then_some(())
    }
}

fn emu(px: f64) -> i64 {
    (px * EMU_PER_PX).round() as i64
}

fn body_text(body: &TextBody) -> Vec<String> {
    body.paragraphs
        .iter()
        .map(|paragraph| paragraph.runs.iter().map(|run| run.text.as_str()).collect())
        .collect()
}

fn collect_text(shapes: &[ShapeItem], out: &mut SlideText) {
    for item in shapes {
        match item {
            ShapeItem::Shape(shape) => {
                if let (Some(id), Some(body)) = (shape.id, shape.text.as_ref()) {
                    out.insert(id, body_text(body));
                }
            }
            ShapeItem::Group(group) => collect_text(&group.children, out),
            _ => {}
        }
    }
}

#[cfg(all(test, feature = "tiny-skia"))]
mod tests {
    use super::*;
    use crate::test_support::{faces, fixture};
    use crate::tiny::TinySkiaBackend;

    fn open() -> Deck<TinySkiaBackend> {
        Deck::open(
            &fixture("sample-deck.pptx"),
            TinySkiaBackend::new(&faces()).unwrap(),
        )
        .unwrap()
    }

    fn first_shape(deck: &mut Deck<TinySkiaBackend>) -> u32 {
        deck.shapes(0)
            .first()
            .map(|shape| shape.id)
            .expect("a shape")
    }

    #[test]
    fn an_added_entrance_plays_in_the_show_and_undoes() {
        let mut deck = open();
        let id = first_shape(&mut deck);
        let add = AnimationEdit::Add {
            class: PresetClass::Entrance,
            preset_id: 10,
            params: EffectParams::default(),
            targets: vec![AnimationTarget::shape(id)],
            start: None,
            after: None,
        };
        assert!(deck.edit_animation(0, &add).unwrap());
        assert_eq!(deck.animation_pane(0).len(), 1);
        let mut player = deck.player(0);
        player.start(0.0);
        let key = TargetKey::shape(id);
        assert!(!deck.frame(0, &player, 0.0).state(&key).visible);
        assert_eq!(player.next(0.0), Advance::Animated);
        let middle = deck.frame(0, &player, 250.0).state(&key);
        assert!((middle.opacity - 0.5).abs() < 1e-6);
        let frame = deck.frame(0, &player, 250.0);
        assert!(deck.render_frame(0, 320, &frame).is_some());
        assert_eq!(player.next(1000.0), Advance::NextSlide);
        assert!(deck.undo().unwrap());
        assert!(deck.animation_pane(0).is_empty());
    }

    #[test]
    fn transitions_apply_to_all_and_morph_paints() {
        let mut deck = open();
        let count = deck.slide_count();
        let morph = TRANSITIONS
            .iter()
            .find(|preset| preset.key == "morph")
            .unwrap();
        let all: Vec<usize> = (0..count).collect();
        assert!(deck
            .set_transition(
                &all,
                &TransitionEdit::Effect(Some(preset_effect(morph, None)))
            )
            .unwrap());
        for slide in 0..count {
            let transition = deck.show_transition(slide).expect("a transition");
            assert_eq!(transition.morph, Some(MorphOption::ByObject));
            assert_eq!(transition.duration_ms, 2000);
        }
        if count > 1 {
            assert!(deck
                .render_morph(0, 1, 320, MorphOption::ByWord, 0.5)
                .is_some());
        }
    }

    #[test]
    fn a_slide_zoom_jumps_to_its_slide() {
        let mut deck = open();
        let last = deck.slide_count() - 1;
        let id = deck.deck.slides[last].id.expect("a slide id");
        let ids = deck
            .insert_zoom(0, ZoomKind::Slide, &[ZoomTarget::Slide(id)], (40.0, 40.0))
            .unwrap();
        assert_eq!(ids.len(), 1);
        let zoom = deck
            .zoom_at(0, 50.0, 50.0)
            .expect("the zoom is under the point");
        assert_eq!(deck.zoom_slides(&zoom), vec![last]);
        assert!(deck
            .set_zoom_options(
                0,
                ids[0],
                ZoomOptions {
                    return_to_parent: true,
                    transition_ms: 500,
                    show_background: true,
                },
            )
            .unwrap());
        assert!(deck.zooms(0)[0].return_to_parent);
    }
}

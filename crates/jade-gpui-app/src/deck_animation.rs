//! The アニメーション, 画面切り替え and ズーム panes of the deck view.
//!
//! PowerPoint for Mac's アニメーション tab and アニメーション ウィンドウ in
//! one side pane: the effect galleries (開始, 強調, 終了, アニメーションの
//! 軌跡, with その他の効果), 効果のオプション, アニメーションの追加, the
//! timing (開始, 継続時間, 遅延, 順番), the effect options (スムーズに開始/
//! 終了, 繰り返し, 巻き戻し, アニメーションの後, テキストの動作, 連続,
//! トリガー), アニメーションのコピー/貼り付け, プレビュー and the effect
//! list with its timeline. The 画面切り替え pane is the transition gallery,
//! its options, 継続時間, サウンド, 画面切り替えのタイミング and すべてに
//! 適用; the ズーム pane inserts Summary, Slide and Section Zoom and sets
//! a zoom's options. Everything goes through `jade_office_model`'s deck.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use jade_office_model::deck::animation::{
    compose_transition, preset, AfterEffect, AnimationEdit, AnimationEffect, AnimationTarget,
    ChartSequence, EffectClass, EffectParams, EffectPreset, EffectStart, IterateUnit, PaneEffect,
    PresetClass, Repeat, SlidePlayer, SoundChoice, TargetPart, TextSequence, TransitionEdit,
    TransitionFrame, Trigger, ZoomKind, ZoomOptions, ZoomTarget, PRESETS, TRANSITIONS,
};

use super::*;

/// The pane's width.
pub(super) const PANEL_WIDTH: f32 = 300.0;
/// The timeline's scale in the effect list.
const TIMELINE_PX_PER_SECOND: f32 = 36.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tab {
    Animations,
    Transitions,
    Zoom,
}

/// What a プレビュー is playing.
struct Preview {
    clock: Instant,
    slide: usize,
    player: SlidePlayer,
    /// A transition into the slide: the outgoing slide's pixels.
    transition: Option<(Vec<u8>, u32, u32)>,
    image: Option<Arc<RenderImage>>,
}

pub(super) struct AnimationUi {
    pub(super) tab: Option<Tab>,
    class: PresetClass,
    /// その他の効果: every preset of the class, not only the gallery's.
    more: bool,
    /// アニメーションの追加: a gallery click adds rather than replaces.
    adding: bool,
    /// Effects selected in the list, by their index in it.
    selected: Vec<usize>,
    /// アニメーションのコピー/貼り付け: the effects picked up and their shape.
    painter: Option<(u32, Vec<AnimationEffect>)>,
    preview: Option<Preview>,
    /// ユーザー設定パス being drawn on the preview: slide px.
    drawing: Option<Vec<(f64, f64)>>,
    preview_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    zoom_kind: ZoomKind,
    /// Slides (or sections) ticked in the ズーム pane.
    picks: Vec<usize>,
}

impl Default for AnimationUi {
    fn default() -> Self {
        Self {
            tab: None,
            class: PresetClass::Entrance,
            more: false,
            adding: false,
            selected: Vec::new(),
            painter: None,
            preview: None,
            drawing: None,
            preview_bounds: Rc::new(Cell::new(None)),
            zoom_kind: ZoomKind::Slide,
            picks: Vec::new(),
        }
    }
}

impl AnimationUi {
    /// The room the pane takes beside the slide.
    pub(super) fn width(&self) -> f32 {
        if self.tab.is_some() {
            PANEL_WIDTH
        } else {
            0.0
        }
    }
}

/// The text for `prefix.name`, for names that come from the presets.
fn label(prefix: &str, name: &str) -> &'static str {
    static KEYS: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let key = format!("{prefix}.{name}");
    let interned = {
        let mut keys = KEYS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *keys
            .entry(key.clone())
            .or_insert_with(|| Box::leak(key.into_boxed_str()))
    };
    i18n::text(interned)
}

/// An effect's name as the list shows it.
pub(super) fn effect_name(effect: &AnimationEffect) -> &'static str {
    PresetClass::from_class(&effect.class)
        .and_then(|class| preset(class, effect.preset_id))
        .map_or_else(
            || match effect.class {
                EffectClass::Media => i18n::text("anim.media"),
                _ => i18n::text("anim.custom_effect"),
            },
            |preset| label("anim.effect", preset.name),
        )
}

fn class_color(class: &EffectClass) -> u32 {
    match class {
        EffectClass::Entrance => 0x3BA55C,
        EffectClass::Emphasis => 0xE3B341,
        EffectClass::Exit => 0xE5534B,
        EffectClass::MotionPath => 0x4C8DF6,
        _ => 0x8B8B8B,
    }
}

/// Each listed effect's (click number, start ms in its click, end ms).
pub(super) fn timeline(pane: &[PaneEffect]) -> Vec<(usize, f64, f64)> {
    let mut out = Vec::new();
    let mut click = 0;
    let mut step_start = 0.0;
    let mut step_end: f64 = 0.0;
    let mut trigger: Option<Trigger> = None;
    for (index, item) in pane.iter().enumerate() {
        let effect = &item.effect;
        if item.trigger != trigger {
            trigger = item.trigger.clone();
            step_start = 0.0;
            step_end = 0.0;
        }
        match effect.start {
            EffectStart::OnClick => {
                if item.trigger.is_none() {
                    click += 1;
                }
                step_start = 0.0;
                step_end = 0.0;
            }
            EffectStart::WithPrevious => {}
            EffectStart::AfterPrevious => step_start = step_end,
        }
        if index == 0 && effect.start != EffectStart::OnClick {
            click = 0;
        }
        let begin = step_start + f64::from(effect.delay_ms);
        let end = begin + f64::from(effect.duration_ms);
        step_end = step_end.max(end);
        out.push((click, begin, end));
    }
    out
}

fn segmented(
    id: &'static str,
    index: usize,
    text: impl Into<SharedString>,
    on: bool,
) -> Stateful<Div> {
    div()
        .id((id, index))
        .px_2()
        .py_0p5()
        .rounded_md()
        .text_xs()
        .cursor_pointer()
        .border_1()
        .border_color(theme::border())
        .when(on, |chip| {
            chip.bg(theme::selected()).text_color(theme::text())
        })
        .when(!on, |chip| chip.text_color(theme::text_dim()))
        .hover(|chip| chip.bg(theme::hovered()))
        .child(text.into())
}

fn painter_image(painter: &<SkiaBackend as Backend>::Painter) -> Option<Arc<RenderImage>> {
    let (bgra, width, height) = SkiaBackend::bgra(painter)?;
    let buffer = image::ImageBuffer::from_raw(width, height, bgra)?;
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
}

fn heading(text: &'static str) -> Div {
    div()
        .pt_2()
        .pb_1()
        .text_xs()
        .text_color(theme::text_faint())
        .child(text)
}

fn chips() -> Div {
    div().flex().flex_wrap().gap_1()
}

impl DeckView {
    /// The toolbar's アニメーション, 画面切り替え and ズーム buttons.
    pub(super) fn animation_buttons(&mut self, cx: &mut Context<Self>) -> Div {
        let open = self.anim.tab;
        let button = |id: &'static str, icon: &'static str, key: &'static str, tab: Tab| {
            toolbar_button(id, icon, i18n::text(key))
                .when(open == Some(tab), |button| {
                    button.bg(theme::selected()).text_color(theme::text())
                })
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.anim.tab = if view.anim.tab == Some(tab) {
                        None
                    } else {
                        Some(tab)
                    };
                    view.anim.preview = None;
                    cx.notify();
                }))
        };
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(button(
                "animations-tab",
                "icons/sparkles.svg",
                "anim.tab",
                Tab::Animations,
            ))
            .child(button(
                "transitions-tab",
                "icons/presentation.svg",
                "transition.tab",
                Tab::Transitions,
            ))
            .child(button(
                "zoom-tab",
                "icons/blocks.svg",
                "zoom.tab",
                Tab::Zoom,
            ))
    }

    /// The shapes the アニメーション tab acts on: the selection.
    fn animation_targets(&self) -> Vec<u32> {
        self.selected
            .into_iter()
            .chain(self.more.iter().copied())
            .collect()
    }

    /// The listed effects the options apply to: those selected in the
    /// list, else the selected shapes' own.
    fn effect_indices(&mut self) -> Vec<usize> {
        let pane = self.deck.animation_pane(self.current);
        let chosen: Vec<usize> = self
            .anim
            .selected
            .iter()
            .copied()
            .filter(|&index| index < pane.len())
            .collect();
        if !chosen.is_empty() {
            return chosen;
        }
        let targets = self.animation_targets();
        pane.iter()
            .enumerate()
            .filter(|(_, item)| targets.contains(&item.effect.target.shape_id))
            .map(|(index, _)| index)
            .collect()
    }

    fn edit_anim(&mut self, edit: AnimationEdit, cx: &mut Context<Self>) -> bool {
        let slide = self.current;
        let changed = match self.deck.edit_animation(slide, &edit) {
            Ok(changed) => changed,
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
                false
            }
        };
        cx.notify();
        changed
    }

    fn edit_transition(
        &mut self,
        slides: Vec<usize>,
        edit: TransitionEdit,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.deck.set_transition(&slides, &edit) {
            self.status =
                SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
        }
        cx.notify();
    }

    /// A gallery effect clicked: replaces the selected effects, or adds
    /// one to each selected shape (always, under アニメーションの追加).
    fn choose_effect(&mut self, preset: &'static EffectPreset, cx: &mut Context<Self>) {
        if preset.class == PresetClass::Path && preset.id == 0 {
            // ユーザー設定パス: drawn on the preview, click by click.
            self.anim.drawing = Some(Vec::new());
            self.anim.preview = None;
            self.status = SharedString::from(i18n::text("anim.draw_path_hint"));
            cx.notify();
            return;
        }
        let indices = self.effect_indices();
        let before = self.deck.animation_pane(self.current).len();
        let edit = if !self.anim.adding && !indices.is_empty() {
            AnimationEdit::Change {
                indices,
                class: preset.class,
                preset_id: preset.id,
                params: EffectParams::default(),
            }
        } else {
            let targets = self.animation_targets();
            if targets.is_empty() {
                self.status = SharedString::from(i18n::text("anim.select_object"));
                cx.notify();
                return;
            }
            AnimationEdit::Add {
                class: preset.class,
                preset_id: preset.id,
                params: EffectParams::default(),
                targets: targets.into_iter().map(AnimationTarget::shape).collect(),
                start: None,
                after: None,
            }
        };
        let adding = matches!(edit, AnimationEdit::Add { .. });
        if self.edit_anim(edit, cx) {
            if adding {
                let after = self.deck.animation_pane(self.current).len();
                self.anim.selected = (before..after).collect();
            }
            self.anim.adding = false;
            self.start_preview(false, cx);
        }
    }

    /// The custom path drawn so far, finished: an effect on each selected
    /// shape following it.
    fn finish_path(&mut self, cx: &mut Context<Self>) {
        let Some(points) = self.anim.drawing.take() else {
            return;
        };
        let targets = self.animation_targets();
        if points.len() < 2 || targets.is_empty() {
            cx.notify();
            return;
        }
        let (width, height) = self.deck.slide_size();
        let origin = points[0];
        let path: Vec<(f64, f64)> = points
            .iter()
            .map(|(x, y)| ((x - origin.0) / width, (y - origin.1) / height))
            .collect();
        let closed = {
            let last = points[points.len() - 1];
            ((last.0 - origin.0).powi(2) + (last.1 - origin.1).powi(2)).sqrt() < 8.0
        };
        let before = self.deck.animation_pane(self.current).len();
        let edit = AnimationEdit::Add {
            class: PresetClass::Path,
            preset_id: 0,
            params: EffectParams {
                path: Some(path),
                closed,
                ..EffectParams::default()
            },
            targets: targets.into_iter().map(AnimationTarget::shape).collect(),
            start: None,
            after: None,
        };
        if self.edit_anim(edit, cx) {
            let after = self.deck.animation_pane(self.current).len();
            self.anim.selected = (before..after).collect();
            self.start_preview(false, cx);
        }
    }

    /// プレビュー: the slide's animations (or its transition) played in
    /// the pane.
    fn start_preview(&mut self, transition: bool, cx: &mut Context<Self>) {
        let slide = self.current;
        let mut player = self.deck.player(slide);
        player.start(0.0);
        let outgoing = if transition {
            let width = (PANEL_WIDTH * 2.0) as u32;
            let from = slide.checked_sub(1);
            match from.and_then(|from| self.deck.render_slide(from, width)) {
                Some(painter) => SkiaBackend::bgra(&painter),
                None => self.deck.render_slide(slide, width).and_then(|painter| {
                    SkiaBackend::bgra(&painter).map(|(pixels, w, h)| (vec![0; pixels.len()], w, h))
                }),
            }
        } else {
            None
        };
        self.anim.preview = Some(Preview {
            clock: Instant::now(),
            slide,
            player,
            transition: outgoing,
            image: None,
        });
        cx.notify();
    }

    /// The preview's frame now, asking for the next one while it plays.
    fn preview_frame(&mut self, window: &mut Window) -> Option<Arc<RenderImage>> {
        let device_width = ((PANEL_WIDTH - 24.0) * window.scale_factor()).round() as u32;
        let slide = self.current;
        let preview = self.anim.preview.as_mut().filter(|p| p.slide == slide);
        let Some(preview) = preview else {
            let painter = self.deck.render_slide(slide, device_width)?;
            return painter_image(&painter);
        };
        let now = preview.clock.elapsed().as_secs_f64() * 1000.0;
        if let Some((pixels, width, height)) = &preview.transition {
            let transition = self.deck.show_transition(slide);
            let duration = transition
                .as_ref()
                .map_or(0.0, |t| f64::from(t.duration_ms));
            if now < duration {
                window.request_animation_frame();
                let progress = now / duration.max(1.0);
                let transition = transition?;
                let image =
                    if let (Some(option), Some(from)) = (transition.morph, slide.checked_sub(1)) {
                        let painter = self
                            .deck
                            .render_morph(from, slide, *width, option, progress)?;
                        painter_image(&painter)
                    } else {
                        let frame = self.deck.frame(slide, &preview.player, 0.0);
                        let painter = self.deck.render_frame(slide, *width, &frame)?;
                        let (to, to_width, to_height) = SkiaBackend::bgra(&painter)?;
                        if (to_width, to_height) != (*width, *height) {
                            return painter_image(&painter);
                        }
                        let blended = compose_transition(
                            TransitionFrame {
                                from: pixels,
                                to: &to,
                                width: to_width,
                                height: to_height,
                            },
                            &transition.effect,
                            progress,
                        );
                        let buffer = image::ImageBuffer::from_raw(to_width, to_height, blended)?;
                        Some(Arc::new(RenderImage::new(vec![image::Frame::new(buffer)])))
                    };
                if let Some(old) = std::mem::replace(&mut preview.image, image.clone()) {
                    self.retired.push(old);
                }
                return image;
            }
            preview.transition = None;
            preview.player.start(now);
        }
        // Click groups follow one another, as プレビュー plays them.
        if !preview.player.is_animating(now) && preview.player.remaining_clicks() > 0 {
            preview.player.next(now);
        }
        if preview.player.is_animating(now) {
            window.request_animation_frame();
        }
        let player = preview.player.clone();
        let frame = self.deck.frame(slide, &player, now);
        let painter = self.deck.render_frame(slide, device_width, &frame)?;
        let image = painter_image(&painter);
        let preview = self.anim.preview.as_mut()?;
        if let Some(old) = std::mem::replace(&mut preview.image, image.clone()) {
            self.retired.push(old);
        }
        image
    }

    /// The pane, when one is open.
    pub(super) fn animation_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let tab = self.anim.tab?;
        // アニメーションのコピー/貼り付け lands on the next shape selected.
        if let Some((source, effects)) = self.anim.painter.clone() {
            let targets: Vec<u32> = self
                .animation_targets()
                .into_iter()
                .filter(|&id| id != source)
                .collect();
            if !targets.is_empty() {
                self.anim.painter = None;
                self.edit_anim(
                    AnimationEdit::Paint {
                        source: effects,
                        targets,
                    },
                    cx,
                );
            }
        }
        let image = self.preview_frame(window);
        let bounds_store = Rc::clone(&self.anim.preview_bounds);
        let drawing = self.anim.drawing.clone();
        let (slide_width, slide_height) = self.deck.slide_size();
        let preview_width = PANEL_WIDTH - 24.0;
        let preview_height = preview_width * (slide_height / slide_width.max(1.0)) as f32;
        let path_overlay =
            drawing.as_ref().map(|points| {
                let scale = preview_width / slide_width.max(1.0) as f32;
                div().absolute().top_0().left_0().size_full().children(
                    points.iter().enumerate().map(move |(index, (x, y))| {
                        div()
                            .id(("path-point", index))
                            .absolute()
                            .left(px(*x as f32 * scale - 3.0))
                            .top(px(*y as f32 * scale - 3.0))
                            .size(px(6.0))
                            .rounded_full()
                            .bg(rgb(0x4C8DF6))
                    }),
                )
            });
        let preview = div()
            .id("anim-preview")
            .relative()
            .w(px(preview_width))
            .h(px(preview_height))
            .border_1()
            .border_color(theme::border())
            .bg(gpui::white())
            .when(drawing.is_some(), |preview| {
                preview.cursor(gpui::CursorStyle::Crosshair)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |view, event: &MouseDownEvent, _window, cx| {
                    let Some(bounds) = view.anim.preview_bounds.get() else {
                        return;
                    };
                    if view.anim.drawing.is_none() {
                        return;
                    }
                    let scale = f64::from(f32::from(bounds.size.width)) / slide_width.max(1.0);
                    let x = f64::from(f32::from(event.position.x - bounds.origin.x)) / scale;
                    let y = f64::from(f32::from(event.position.y - bounds.origin.y)) / scale;
                    if let Some(points) = view.anim.drawing.as_mut() {
                        points.push((x, y));
                    }
                    if event.click_count >= 2 {
                        view.finish_path(cx);
                    }
                    cx.notify();
                }),
            )
            .children(image.map(|image| img(image).w(px(preview_width)).h(px(preview_height))))
            .children(path_overlay)
            .child(
                canvas(
                    move |bounds, _window, _cx| {
                        bounds_store.set(Some(bounds));
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        let body = match tab {
            Tab::Animations => self.animations_body(cx),
            Tab::Transitions => self.transitions_body(cx),
            Tab::Zoom => self.zoom_body(cx),
        };
        let title = match tab {
            Tab::Animations => "anim.tab",
            Tab::Transitions => "transition.tab",
            Tab::Zoom => "zoom.tab",
        };
        Some(
            div()
                .id("animation-pane")
                .w(px(PANEL_WIDTH))
                .h_full()
                .flex_none()
                .flex()
                .flex_col()
                .bg(theme::sidebar())
                .border_l_1()
                .border_color(theme::border())
                .overflow_y_scroll()
                .px_3()
                .pb_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .pt_3()
                        .pb_2()
                        .child(div().text_sm().child(i18n::text(title)))
                        .child(
                            div()
                                .id("preview-button")
                                .flex()
                                .items_center()
                                .gap_1()
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .text_xs()
                                .cursor_pointer()
                                .hover(|button| button.bg(theme::hovered()))
                                .child(
                                    svg()
                                        .path("icons/play.svg")
                                        .size_3()
                                        .text_color(theme::text_dim()),
                                )
                                .child(i18n::text("anim.preview"))
                                .on_click(cx.listener(move |view, _event, _window, cx| {
                                    view.start_preview(tab == Tab::Transitions, cx)
                                })),
                        ),
                )
                .child(preview)
                .when(drawing.is_some(), |pane| {
                    pane.child(
                        div()
                            .flex()
                            .gap_1()
                            .pt_1()
                            .child(
                                segmented("path-done", 0, i18n::text("anim.path_done"), true)
                                    .on_click(cx.listener(|view, _event, _window, cx| {
                                        view.finish_path(cx)
                                    })),
                            )
                            .child(
                                segmented("path-cancel", 0, i18n::text("anim.path_cancel"), false)
                                    .on_click(cx.listener(|view, _event, _window, cx| {
                                        view.anim.drawing = None;
                                        cx.notify();
                                    })),
                            ),
                    )
                })
                .child(body)
                .into_any_element(),
        )
    }

    fn animations_body(&mut self, cx: &mut Context<Self>) -> Div {
        let class = self.anim.class;
        let more = self.anim.more;
        let pane = self.deck.animation_pane(self.current);
        let indices = self.effect_indices();
        let current: Option<AnimationEffect> = indices
            .first()
            .and_then(|&index| pane.get(index))
            .map(|item| item.effect.clone());
        let classes = [
            (PresetClass::Entrance, "anim.class.entrance"),
            (PresetClass::Emphasis, "anim.class.emphasis"),
            (PresetClass::Exit, "anim.class.exit"),
            (PresetClass::Path, "anim.class.path"),
        ];
        let class_row =
            chips().children(classes.into_iter().enumerate().map(|(i, (value, key))| {
                segmented("anim-class", i, i18n::text(key), class == value).on_click(cx.listener(
                    move |view, _event, _window, cx| {
                        view.anim.class = value;
                        cx.notify();
                    },
                ))
            }));
        let presets: Vec<(usize, &'static EffectPreset)> = PRESETS
            .iter()
            .enumerate()
            .filter(|(_, preset)| preset.class == class && (more || preset.gallery))
            .collect();
        let gallery = chips().children(presets.into_iter().map(|(i, preset)| {
            let on = current.as_ref().is_some_and(|effect| {
                effect.preset_id == preset.id
                    && PresetClass::from_class(&effect.class) == Some(preset.class)
            });
            div()
                .id(("anim-preset", i))
                .w(px(84.0))
                .h(px(40.0))
                .flex()
                .items_center()
                .justify_center()
                .text_center()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .bg(theme::panel())
                .hover(|tile| tile.bg(theme::hovered()))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .child(
                            div()
                                .size(px(8.0))
                                .rounded_full()
                                .bg(rgb(class_color(&preset.class.effect_class()))),
                        )
                        .child(label("anim.effect", preset.name)),
                )
                .on_click(
                    cx.listener(move |view, _event, _window, cx| view.choose_effect(preset, cx)),
                )
        }));
        let toggles = chips()
            .child(
                segmented("anim-more", 0, i18n::text("anim.more_effects"), more).on_click(
                    cx.listener(|view, _event, _window, cx| {
                        view.anim.more = !view.anim.more;
                        cx.notify();
                    }),
                ),
            )
            .child(
                segmented("anim-add", 0, i18n::text("anim.add"), self.anim.adding).on_click(
                    cx.listener(|view, _event, _window, cx| {
                        view.anim.adding = !view.anim.adding;
                        cx.notify();
                    }),
                ),
            )
            .child(
                segmented(
                    "anim-painter",
                    0,
                    i18n::text("anim.painter"),
                    self.anim.painter.is_some(),
                )
                .on_click(cx.listener(|view, _event, _window, cx| {
                    if view.anim.painter.take().is_none() {
                        let indices = view.effect_indices();
                        let pane = view.deck.animation_pane(view.current);
                        let effects: Vec<AnimationEffect> = indices
                            .iter()
                            .filter_map(|&index| pane.get(index))
                            .map(|item| item.effect.clone())
                            .collect();
                        if let (Some(first), false) = (effects.first(), effects.is_empty()) {
                            view.anim.painter = Some((first.target.shape_id, effects));
                            view.status = SharedString::from(i18n::text("anim.painter_hint"));
                        }
                    }
                    cx.notify();
                })),
            );
        let mut body = div()
            .flex()
            .flex_col()
            .child(heading(i18n::text("anim.gallery")))
            .child(class_row)
            .child(div().h(px(6.0)))
            .child(gallery)
            .child(div().h(px(6.0)))
            .child(toggles);
        if let Some(effect) = current.clone() {
            body = body.child(self.effect_options(&effect, &indices, cx));
        }
        body.child(self.animation_list(&pane, cx))
    }

    /// 効果のオプション, タイミング and the effect options dialog's
    /// settings for the selected effects.
    fn effect_options(
        &mut self,
        effect: &AnimationEffect,
        indices: &[usize],
        cx: &mut Context<Self>,
    ) -> Div {
        let indices: Vec<usize> = indices.to_vec();
        let options = PresetClass::from_class(&effect.class)
            .and_then(|class| preset(class, effect.preset_id))
            .map_or(&[][..], |preset| preset.options);
        let subtype = effect.preset_subtype;
        let mut section = div().flex().flex_col();
        if !options.is_empty() {
            let list = indices.clone();
            section = section
                .child(heading(i18n::text("anim.effect_options")))
                .child(
                    chips().children(options.iter().enumerate().map(|(i, (value, key))| {
                        let list = list.clone();
                        let value = *value;
                        segmented(
                            "anim-option",
                            i,
                            label("anim.option", key),
                            subtype == value,
                        )
                        .on_click(cx.listener(
                            move |view, _event, _window, cx| {
                                view.edit_anim(
                                    AnimationEdit::SetOptions {
                                        indices: list.clone(),
                                        params: EffectParams {
                                            subtype: Some(value),
                                            ..EffectParams::default()
                                        },
                                    },
                                    cx,
                                );
                            },
                        ))
                    })),
                );
        }
        // 連続: a text shape's paragraphs, a chart's series or categories.
        let shape_id = effect.target.shape_id;
        let first = indices[0];
        let paragraphs = self
            .deck
            .slide_text(self.current)
            .get(&shape_id)
            .map_or(0, Vec::len) as u32;
        let chart = self.deck.chart_size(self.current, shape_id);
        if paragraphs > 1 {
            let by_paragraph = matches!(effect.target.part, TargetPart::Paragraphs { .. });
            let sequences = [
                (
                    TextSequence::AsOneObject,
                    "anim.seq.one_object",
                    !by_paragraph,
                ),
                (
                    TextSequence::ByParagraph,
                    "anim.seq.by_paragraph",
                    by_paragraph,
                ),
            ];
            section = section.child(heading(i18n::text("anim.sequence"))).child(
                chips().children(sequences.into_iter().enumerate().map(
                    |(i, (sequence, key, on))| {
                        segmented("anim-seq", i, i18n::text(key), on).on_click(cx.listener(
                            move |view, _event, _window, cx| {
                                view.anim.selected.clear();
                                view.edit_anim(
                                    AnimationEdit::SetSequence {
                                        index: first,
                                        sequence,
                                        paragraphs,
                                    },
                                    cx,
                                );
                            },
                        ))
                    },
                )),
            );
        } else if let Some((series, categories)) = chart {
            let sequences = [
                (ChartSequence::AsOneObject, "anim.seq.one_object"),
                (ChartSequence::BySeries, "anim.seq.by_series"),
                (ChartSequence::ByCategory, "anim.seq.by_category"),
                (ChartSequence::BySeriesElement, "anim.seq.by_series_element"),
                (
                    ChartSequence::ByCategoryElement,
                    "anim.seq.by_category_element",
                ),
            ];
            section =
                section.child(heading(i18n::text("anim.sequence"))).child(
                    chips().children(sequences.into_iter().enumerate().map(
                        |(i, (sequence, key))| {
                            segmented("anim-chart-seq", i, i18n::text(key), false).on_click(
                                cx.listener(move |view, _event, _window, cx| {
                                    view.anim.selected.clear();
                                    view.edit_anim(
                                        AnimationEdit::SetChartSequence {
                                            index: first,
                                            sequence,
                                            series,
                                            categories,
                                        },
                                        cx,
                                    );
                                }),
                            )
                        },
                    )),
                );
        }
        // タイミング.
        let starts = [
            (EffectStart::OnClick, "anim.start.on_click"),
            (EffectStart::WithPrevious, "anim.start.with_previous"),
            (EffectStart::AfterPrevious, "anim.start.after_previous"),
        ];
        let list = indices.clone();
        section = section
            .child(heading(i18n::text("anim.start")))
            .child(
                chips().children(starts.into_iter().enumerate().map(|(i, (start, key))| {
                    let list = list.clone();
                    segmented("anim-start", i, i18n::text(key), effect.start == start).on_click(
                        cx.listener(move |view, _event, _window, cx| {
                            view.edit_anim(
                                AnimationEdit::SetStart {
                                    indices: list.clone(),
                                    start,
                                },
                                cx,
                            );
                        }),
                    )
                })),
            );
        let stepper = |id: &'static str,
                       key: &'static str,
                       ms: u32,
                       cx: &mut Context<Self>,
                       make: fn(Vec<usize>, u32) -> AnimationEdit| {
            let down = indices.clone();
            let up = indices.clone();
            div()
                .flex()
                .items_center()
                .gap_2()
                .pt_1()
                .child(
                    div()
                        .w(px(80.0))
                        .text_xs()
                        .text_color(theme::text_dim())
                        .child(i18n::text(key)),
                )
                .child(segmented(id, 0, "−", false).on_click(cx.listener(
                    move |view, _event, _window, cx| {
                        view.edit_anim(make(down.clone(), ms.saturating_sub(250)), cx);
                    },
                )))
                .child(
                    div()
                        .w(px(56.0))
                        .text_xs()
                        .text_center()
                        .child(format!("{:.2}", f64::from(ms) / 1000.0)),
                )
                .child(segmented(id, 1, "+", false).on_click(cx.listener(
                    move |view, _event, _window, cx| {
                        view.edit_anim(make(up.clone(), ms + 250), cx);
                    },
                )))
        };
        section = section
            .child(stepper(
                "anim-duration",
                "anim.duration",
                effect.duration_ms,
                cx,
                |indices, ms| AnimationEdit::SetDuration { indices, ms },
            ))
            .child(stepper(
                "anim-delay",
                "anim.delay",
                effect.delay_ms,
                cx,
                |indices, ms| AnimationEdit::SetDelay { indices, ms },
            ));
        // 順番を前にする / 後にする, 削除.
        let index = indices[0];
        let all = indices.clone();
        section = section.child(
            chips()
                .pt_1()
                .child(
                    segmented("anim-earlier", 0, i18n::text("anim.move_earlier"), false).on_click(
                        cx.listener(move |view, _event, _window, cx| {
                            if index > 0
                                && view.edit_anim(
                                    AnimationEdit::Move {
                                        index,
                                        to: index - 1,
                                    },
                                    cx,
                                )
                            {
                                view.anim.selected = vec![index - 1];
                            }
                        }),
                    ),
                )
                .child(
                    segmented("anim-later", 0, i18n::text("anim.move_later"), false).on_click(
                        cx.listener(move |view, _event, _window, cx| {
                            if view.edit_anim(
                                AnimationEdit::Move {
                                    index,
                                    to: index + 1,
                                },
                                cx,
                            ) {
                                view.anim.selected = vec![index + 1];
                            }
                        }),
                    ),
                )
                .child(
                    segmented("anim-remove", 0, i18n::text("anim.remove"), false).on_click(
                        cx.listener(move |view, _event, _window, cx| {
                            view.anim.selected.clear();
                            view.edit_anim(
                                AnimationEdit::Remove {
                                    indices: all.clone(),
                                },
                                cx,
                            );
                        }),
                    ),
                ),
        );
        // The effect options dialog.
        let smooth = effect.accel > 0.0 || effect.decel > 0.0;
        let list = indices.clone();
        let accel = effect.accel;
        let decel = effect.decel;
        section = section
            .child(heading(i18n::text("anim.enhancements")))
            .child(
                chips()
                    .child(
                        segmented(
                            "anim-smooth-start",
                            0,
                            i18n::text("anim.smooth_start"),
                            accel > 0.0,
                        )
                        .on_click({
                            let list = list.clone();
                            cx.listener(move |view, _event, _window, cx| {
                                view.edit_anim(
                                    AnimationEdit::SetSmoothing {
                                        indices: list.clone(),
                                        accel: if accel > 0.0 { 0.0 } else { 0.5 },
                                        decel,
                                    },
                                    cx,
                                );
                            })
                        }),
                    )
                    .child(
                        segmented(
                            "anim-smooth-end",
                            0,
                            i18n::text("anim.smooth_end"),
                            decel > 0.0,
                        )
                        .on_click({
                            let list = list.clone();
                            cx.listener(move |view, _event, _window, cx| {
                                view.edit_anim(
                                    AnimationEdit::SetSmoothing {
                                        indices: list.clone(),
                                        accel,
                                        decel: if decel > 0.0 { 0.0 } else { 0.5 },
                                    },
                                    cx,
                                );
                            })
                        }),
                    )
                    .child(
                        segmented("anim-rewind", 0, i18n::text("anim.rewind"), effect.rewind)
                            .on_click({
                                let list = list.clone();
                                let rewind = !effect.rewind;
                                cx.listener(move |view, _event, _window, cx| {
                                    view.edit_anim(
                                        AnimationEdit::SetRewind {
                                            indices: list.clone(),
                                            rewind,
                                        },
                                        cx,
                                    );
                                })
                            }),
                    )
                    .child(
                        segmented(
                            "anim-reverse",
                            0,
                            i18n::text("anim.auto_reverse"),
                            effect.auto_reverse,
                        )
                        .on_click({
                            let list = list.clone();
                            let auto_reverse = !effect.auto_reverse;
                            cx.listener(move |view, _event, _window, cx| {
                                view.edit_anim(
                                    AnimationEdit::SetAutoReverse {
                                        indices: list.clone(),
                                        auto_reverse,
                                    },
                                    cx,
                                );
                            })
                        }),
                    ),
            );
        let _ = smooth;
        let repeats = [
            (Repeat::None, "anim.repeat.none"),
            (Repeat::Count(2.0), "anim.repeat.two"),
            (Repeat::Count(3.0), "anim.repeat.three"),
            (Repeat::UntilNextClick, "anim.repeat.next_click"),
            (Repeat::UntilEndOfSlide, "anim.repeat.end_of_slide"),
        ];
        section = section
            .child(heading(i18n::text("anim.repeat")))
            .child(
                chips().children(repeats.into_iter().enumerate().map(|(i, (repeat, key))| {
                    let list = list.clone();
                    segmented("anim-repeat", i, i18n::text(key), effect.repeat == repeat).on_click(
                        cx.listener(move |view, _event, _window, cx| {
                            view.edit_anim(
                                AnimationEdit::SetRepeat {
                                    indices: list.clone(),
                                    repeat,
                                },
                                cx,
                            );
                        }),
                    )
                })),
            );
        let afters = [
            (AfterEffect::None, "anim.after.none"),
            (AfterEffect::Dim("A6A6A6".to_string()), "anim.after.dim"),
            (AfterEffect::Hide, "anim.after.hide"),
            (AfterEffect::HideOnNextClick, "anim.after.hide_on_click"),
        ];
        section = section
            .child(heading(i18n::text("anim.after")))
            .child(
                chips().children(afters.into_iter().enumerate().map(|(i, (after, key))| {
                    let list = list.clone();
                    let on =
                        std::mem::discriminant(&effect.after) == std::mem::discriminant(&after);
                    segmented("anim-after", i, i18n::text(key), on).on_click(cx.listener(
                        move |view, _event, _window, cx| {
                            view.edit_anim(
                                AnimationEdit::SetAfter {
                                    indices: list.clone(),
                                    after: after.clone(),
                                },
                                cx,
                            );
                        },
                    ))
                })),
            );
        if paragraphs > 0 {
            let unit = effect.iterate.as_ref().map(|iterate| iterate.unit);
            let units = [
                (None, "anim.text.all_at_once"),
                (Some(IterateUnit::Word), "anim.text.by_word"),
                (Some(IterateUnit::Letter), "anim.text.by_letter"),
            ];
            section = section
                .child(heading(i18n::text("anim.text")))
                .child(
                    chips().children(units.into_iter().enumerate().map(|(i, (value, key))| {
                        let list = list.clone();
                        let on = unit.filter(|u| *u != IterateUnit::Element) == value;
                        segmented("anim-text", i, i18n::text(key), on).on_click(cx.listener(
                            move |view, _event, _window, cx| {
                                view.edit_anim(
                                    AnimationEdit::SetTextUnit {
                                        indices: list.clone(),
                                        unit: value.map(|unit| (unit, 0.1)),
                                    },
                                    cx,
                                );
                            },
                        ))
                    })),
                );
        }
        // トリガー: クリック時 of another object, or the click sequence.
        let others: Vec<(u32, String)> = self
            .shapes
            .iter()
            .filter(|shape| shape.id != shape_id)
            .map(|shape| (shape.id, shape.name.clone()))
            .collect();
        let trigger = self
            .deck
            .animation_pane(self.current)
            .get(first)
            .and_then(|item| item.trigger.clone());
        let mut trigger_row = chips().child(
            segmented(
                "anim-trigger",
                0,
                i18n::text("anim.trigger.sequence"),
                trigger.is_none(),
            )
            .on_click({
                let list = list.clone();
                cx.listener(move |view, _event, _window, cx| {
                    view.edit_anim(
                        AnimationEdit::SetTrigger {
                            indices: list.clone(),
                            trigger: None,
                        },
                        cx,
                    );
                })
            }),
        );
        for (i, (id, name)) in others.into_iter().enumerate() {
            let list = list.clone();
            let on = matches!(&trigger, Some(Trigger::ShapeClick { shape_id }) if *shape_id == id);
            let text = i18n::format("anim.trigger.on_click_of", &[("name", &name)]);
            trigger_row = trigger_row.child(segmented("anim-trigger-shape", i, text, on).on_click(
                cx.listener(move |view, _event, _window, cx| {
                    view.edit_anim(
                        AnimationEdit::SetTrigger {
                            indices: list.clone(),
                            trigger: Some(Trigger::ShapeClick { shape_id: id }),
                        },
                        cx,
                    );
                }),
            ));
        }
        if let Some(Trigger::Bookmark { bookmark, .. }) = &trigger {
            let text = i18n::format("anim.trigger.bookmark", &[("name", bookmark)]);
            trigger_row = trigger_row.child(segmented("anim-trigger-bookmark", 0, text, true));
        }
        section
            .child(heading(i18n::text("anim.trigger")))
            .child(trigger_row)
    }

    /// The アニメーション ウィンドウ: every effect, its click, name, target
    /// and a bar on the timeline.
    fn animation_list(&mut self, pane: &[PaneEffect], cx: &mut Context<Self>) -> Div {
        let times = timeline(pane);
        let names: HashMap<u32, String> = self
            .shapes
            .iter()
            .map(|shape| (shape.id, shape.name.clone()))
            .collect();
        let selected = self.anim.selected.clone();
        let mut list = div().flex().flex_col().gap_0p5().child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(heading(i18n::text("anim.pane")))
                .child(
                    segmented("anim-remove-all", 0, i18n::text("anim.remove_all"), false).on_click(
                        cx.listener(|view, _event, _window, cx| {
                            view.anim.selected.clear();
                            view.edit_anim(AnimationEdit::RemoveAll { shape_id: None }, cx);
                        }),
                    ),
                ),
        );
        if pane.is_empty() {
            return list.child(
                div()
                    .text_xs()
                    .text_color(theme::text_faint())
                    .child(i18n::text("anim.none")),
            );
        }
        let mut last_trigger: Option<Trigger> = None;
        for (index, item) in pane.iter().enumerate() {
            if item.trigger != last_trigger {
                if let Some(Trigger::ShapeClick { shape_id }) = &item.trigger {
                    let name = names.get(shape_id).cloned().unwrap_or_default();
                    list = list.child(
                        div()
                            .pt_1()
                            .text_xs()
                            .text_color(theme::text_faint())
                            .child(i18n::format("anim.trigger.heading", &[("name", &name)])),
                    );
                }
                last_trigger = item.trigger.clone();
            }
            let (click, begin, end) = times[index];
            let effect = &item.effect;
            let target = names
                .get(&effect.target.shape_id)
                .cloned()
                .unwrap_or_default();
            let target = match effect.target.part {
                TargetPart::Paragraphs { start, .. } => format!("{target} ¶{}", start + 1),
                _ => target,
            };
            let on = selected.contains(&index);
            let bar_left = (begin / 1000.0) as f32 * TIMELINE_PX_PER_SECOND;
            let bar_width = (((end - begin) / 1000.0) as f32 * TIMELINE_PX_PER_SECOND).max(3.0);
            let number = if effect.start == EffectStart::OnClick && item.trigger.is_none() {
                click.to_string()
            } else {
                String::new()
            };
            let after_previous = effect.start == EffectStart::AfterPrevious;
            list = list.child(
                div()
                    .id(("anim-item", index))
                    .flex()
                    .flex_col()
                    .px_1()
                    .py_0p5()
                    .rounded_md()
                    .cursor_pointer()
                    .when(on, |row| row.bg(theme::selected()))
                    .hover(|row| row.bg(theme::hovered()))
                    .on_click(cx.listener(move |view, event: &ClickEvent, _window, cx| {
                        let extend = event.modifiers().platform || event.modifiers().shift;
                        if extend {
                            if let Some(at) = view.anim.selected.iter().position(|&i| i == index) {
                                view.anim.selected.remove(at);
                            } else {
                                view.anim.selected.push(index);
                            }
                        } else {
                            view.anim.selected = vec![index];
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .child(
                                div()
                                    .w(px(14.0))
                                    .text_color(theme::text_faint())
                                    .child(number),
                            )
                            .child(div().w(px(12.0)).when(after_previous, |mark| {
                                mark.child(
                                    svg()
                                        .path("icons/clock.svg")
                                        .size_3()
                                        .text_color(theme::text_faint()),
                                )
                            }))
                            .child(
                                div()
                                    .size(px(8.0))
                                    .rounded_full()
                                    .bg(rgb(class_color(&effect.class))),
                            )
                            .child(div().child(effect_name(effect)))
                            .child(div().text_color(theme::text_dim()).child(target)),
                    )
                    .child(
                        div().h(px(6.0)).relative().ml(px(30.0)).child(
                            div()
                                .absolute()
                                .left(px(bar_left))
                                .w(px(bar_width))
                                .h(px(6.0))
                                .rounded_sm()
                                .bg(rgb(class_color(&effect.class))),
                        ),
                    ),
            );
        }
        list
    }

    fn transitions_body(&mut self, cx: &mut Context<Self>) -> Div {
        let slide = self.current;
        let transition = self.deck.transition(slide);
        let effect = transition.as_ref().and_then(|t| t.effect.clone());
        let chosen = effect
            .as_ref()
            .and_then(jade_office_model::deck::animation::transition_preset);
        let mut body = div().flex().flex_col();
        let groups = [
            (0u8, "transition.group.subtle"),
            (1, "transition.group.exciting"),
            (2, "transition.group.dynamic"),
        ];
        body = body.child(heading(i18n::text("transition.gallery"))).child(
            chips().child(
                segmented(
                    "transition-none",
                    0,
                    i18n::text("transition.none"),
                    effect.is_none(),
                )
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.edit_transition(vec![slide], TransitionEdit::Effect(None), cx);
                })),
            ),
        );
        for (group, key) in groups {
            body = body.child(heading(i18n::text(key))).child(
                chips().children(
                    TRANSITIONS
                        .iter()
                        .enumerate()
                        .filter(|(_, preset)| preset.group == group)
                        .map(|(i, preset)| {
                            let on = chosen.is_some_and(|c| std::ptr::eq(c, preset));
                            segmented(
                                "transition-preset",
                                i,
                                label("transition.effect", preset.key),
                                on,
                            )
                            .on_click(cx.listener(
                                move |view, _event, _window, cx| {
                                    let effect = jade_office_model::deck::animation::preset_effect(
                                        preset, None,
                                    );
                                    view.edit_transition(
                                        vec![slide],
                                        TransitionEdit::Effect(Some(effect)),
                                        cx,
                                    );
                                    view.start_preview(true, cx);
                                },
                            ))
                        }),
                ),
            );
        }
        if let Some(preset) = chosen.filter(|preset| !preset.options.is_empty()) {
            let current = effect.clone();
            body =
                body.child(heading(i18n::text("anim.effect_options")))
                    .child(chips().children(preset.options.iter().enumerate().map(
                        |(i, (attr, value, key))| {
                            let on = current.as_ref().is_some_and(|effect| {
                                effect.attr(attr) == Some(*value) || (effect.name == *value)
                            });
                            segmented("transition-option", i, label("anim.option", key), on)
                                .on_click(cx.listener(move |view, _event, _window, cx| {
                                    let effect = jade_office_model::deck::animation::preset_effect(
                                        preset,
                                        Some((*attr, *value)),
                                    );
                                    view.edit_transition(
                                        vec![slide],
                                        TransitionEdit::Effect(Some(effect)),
                                        cx,
                                    );
                                    view.start_preview(true, cx);
                                }))
                        },
                    )));
        }
        let duration = transition
            .as_ref()
            .map_or(chosen.map_or(1000, |p| p.duration_ms), |t| {
                t.effective_duration_ms()
            });
        body = body.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .pt_2()
                .child(
                    div()
                        .w(px(80.0))
                        .text_xs()
                        .text_color(theme::text_dim())
                        .child(i18n::text("anim.duration")),
                )
                .child(
                    segmented("transition-duration", 0, "−", false).on_click(cx.listener(
                        move |view, _event, _window, cx| {
                            view.edit_transition(
                                vec![slide],
                                TransitionEdit::Duration(duration.saturating_sub(250).max(10)),
                                cx,
                            );
                        },
                    )),
                )
                .child(
                    div()
                        .w(px(56.0))
                        .text_xs()
                        .text_center()
                        .child(format!("{:.2}", f64::from(duration) / 1000.0)),
                )
                .child(
                    segmented("transition-duration", 1, "+", false).on_click(cx.listener(
                        move |view, _event, _window, cx| {
                            view.edit_transition(
                                vec![slide],
                                TransitionEdit::Duration(duration + 250),
                                cx,
                            );
                        },
                    )),
                ),
        );
        // サウンド.
        let sound = transition.as_ref().and_then(|t| t.sound.clone());
        let stop = transition.as_ref().is_some_and(|t| t.stop_previous_sound);
        let sound_name = sound.as_ref().map(|s| s.name.clone());
        let mut sound_row = chips()
            .child(
                segmented(
                    "transition-sound",
                    0,
                    i18n::text("transition.sound.none"),
                    sound.is_none() && !stop,
                )
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.edit_transition(vec![slide], TransitionEdit::Sound(SoundChoice::None), cx);
                })),
            )
            .child(
                segmented(
                    "transition-sound",
                    1,
                    i18n::text("transition.sound.stop"),
                    stop,
                )
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.edit_transition(
                        vec![slide],
                        TransitionEdit::Sound(SoundChoice::StopPrevious),
                        cx,
                    );
                })),
            )
            .child(
                segmented(
                    "transition-sound",
                    2,
                    i18n::text("transition.sound.other"),
                    false,
                )
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.pick_transition_sound(slide, cx)
                })),
            );
        if let Some(name) = sound_name {
            sound_row = sound_row.child(segmented("transition-sound", 3, name, true));
            let looping = sound.as_ref().is_some_and(|s| s.loop_until_next);
            sound_row = sound_row.child(
                segmented(
                    "transition-loop",
                    0,
                    i18n::text("transition.sound.loop"),
                    looping,
                )
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.edit_transition(vec![slide], TransitionEdit::LoopSound(!looping), cx);
                })),
            );
        }
        body = body
            .child(heading(i18n::text("transition.sound")))
            .child(sound_row);
        // 画面切り替えのタイミング.
        let on_click = transition.as_ref().is_none_or(|t| t.advance_on_click);
        let after = transition.as_ref().and_then(|t| t.advance_after_ms);
        body = body.child(heading(i18n::text("transition.advance"))).child(
            chips()
                .child(
                    segmented(
                        "transition-on-click",
                        0,
                        i18n::text("transition.on_click"),
                        on_click,
                    )
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.edit_transition(
                            vec![slide],
                            TransitionEdit::AdvanceOnClick(!on_click),
                            cx,
                        );
                    })),
                )
                .child(
                    segmented(
                        "transition-after",
                        0,
                        i18n::text("transition.after"),
                        after.is_some(),
                    )
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        let next = if after.is_some() { None } else { Some(5000) };
                        view.edit_transition(vec![slide], TransitionEdit::AdvanceAfter(next), cx);
                    })),
                ),
        );
        if let Some(after) = after {
            body =
                body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .pt_1()
                        .child(
                            div()
                                .w(px(80.0))
                                .text_xs()
                                .text_color(theme::text_dim())
                                .child(i18n::text("transition.after")),
                        )
                        .child(segmented("transition-after-step", 0, "−", false).on_click(
                            cx.listener(move |view, _event, _window, cx| {
                                view.edit_transition(
                                    vec![slide],
                                    TransitionEdit::AdvanceAfter(Some(after.saturating_sub(1000))),
                                    cx,
                                );
                            }),
                        ))
                        .child(
                            div()
                                .w(px(56.0))
                                .text_xs()
                                .text_center()
                                .child(format!("{:.2}", f64::from(after) / 1000.0)),
                        )
                        .child(segmented("transition-after-step", 1, "+", false).on_click(
                            cx.listener(move |view, _event, _window, cx| {
                                view.edit_transition(
                                    vec![slide],
                                    TransitionEdit::AdvanceAfter(Some(after + 1000)),
                                    cx,
                                );
                            }),
                        )),
                );
        }
        let count = self.deck.slide_count();
        body.child(div().h(px(8.0))).child(
            segmented(
                "transition-apply-all",
                0,
                i18n::text("transition.apply_all"),
                false,
            )
            .on_click(cx.listener(move |view, _event, _window, cx| {
                if let Some(transition) = view.deck.transition(slide) {
                    view.edit_transition(
                        (0..count).collect(),
                        TransitionEdit::Replace(transition),
                        cx,
                    );
                }
            })),
        )
    }

    fn pick_transition_sound(&mut self, slide: usize, cx: &mut Context<Self>) {
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
                Ok(bytes) => {
                    let name = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    view.edit_transition(
                        vec![slide],
                        TransitionEdit::Sound(SoundChoice::File { name, bytes }),
                        cx,
                    );
                }
                Err(error) => {
                    let error = format!("{}: {error}", path.display());
                    view.status =
                        SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn zoom_body(&mut self, cx: &mut Context<Self>) -> Div {
        let kind = self.anim.zoom_kind;
        let kinds = [
            (ZoomKind::Summary, "zoom.summary"),
            (ZoomKind::Slide, "zoom.slide"),
            (ZoomKind::Section, "zoom.section"),
        ];
        let mut body = div()
            .flex()
            .flex_col()
            .child(heading(i18n::text("zoom.insert")))
            .child(
                chips().children(kinds.into_iter().enumerate().map(|(i, (value, key))| {
                    segmented("zoom-kind", i, i18n::text(key), kind == value).on_click(cx.listener(
                        move |view, _event, _window, cx| {
                            view.anim.zoom_kind = value;
                            view.anim.picks.clear();
                            cx.notify();
                        },
                    ))
                })),
            );
        let sections = self.deck.sections();
        let choices: Vec<String> = match kind {
            ZoomKind::Slide => (0..self.deck.slide_count())
                .map(|index| i18n::format("deck.slide_n", &[("n", &(index + 1))]))
                .collect(),
            _ => sections
                .iter()
                .map(|section| section.name.clone())
                .collect(),
        };
        if choices.is_empty() {
            body = body.child(
                div()
                    .pt_1()
                    .text_xs()
                    .text_color(theme::text_faint())
                    .child(i18n::text("zoom.no_sections")),
            );
        }
        let picks = self.anim.picks.clone();
        body = body.child(div().flex().flex_col().gap_0p5().pt_1().children(
            choices.into_iter().enumerate().map(|(index, name)| {
                let on = picks.contains(&index);
                div()
                    .id(("zoom-pick", index))
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_1()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .hover(|row| row.bg(theme::hovered()))
                    .child(
                        div()
                            .size(px(12.0))
                            .border_1()
                            .border_color(theme::border())
                            .when(on, |box_| box_.bg(theme::accent())),
                    )
                    .child(name)
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        if let Some(at) = view.anim.picks.iter().position(|&p| p == index) {
                            view.anim.picks.remove(at);
                        } else {
                            view.anim.picks.push(index);
                        }
                        cx.notify();
                    }))
            }),
        ));
        body = body.child(div().h(px(6.0))).child(
            segmented("zoom-insert", 0, i18n::text("zoom.insert_button"), false)
                .on_click(cx.listener(|view, _event, _window, cx| view.insert_zooms(cx))),
        );
        // ズームのオプション for a selected zoom.
        let slide = self.current;
        let selected = self.selected;
        let zoom = selected.and_then(|id| {
            self.deck
                .zooms(slide)
                .into_iter()
                .find(|zoom| zoom.shape_id == id)
        });
        if let Some(zoom) = zoom {
            let options = ZoomOptions {
                return_to_parent: zoom.return_to_parent,
                transition_ms: zoom.transition_ms,
                show_background: !zoom.hide_background,
            };
            let id = zoom.shape_id;
            let set = move |next: ZoomOptions| {
                move |view: &mut DeckView,
                      _event: &ClickEvent,
                      _window: &mut Window,
                      cx: &mut Context<DeckView>| {
                    let result = view.deck.set_zoom_options(slide, id, next);
                    if let Err(error) = result {
                        view.status = SharedString::from(i18n::format(
                            "deck.edit_failed",
                            &[("error", &error)],
                        ));
                    }
                    cx.notify();
                }
            };
            body = body
                .child(heading(i18n::text("zoom.options")))
                .child(
                    chips()
                        .child(
                            segmented(
                                "zoom-return",
                                0,
                                i18n::text("zoom.return"),
                                options.return_to_parent,
                            )
                            .on_click(cx.listener(set(ZoomOptions {
                                return_to_parent: !options.return_to_parent,
                                ..options
                            }))),
                        )
                        .child(
                            segmented(
                                "zoom-background",
                                0,
                                i18n::text("zoom.background"),
                                options.show_background,
                            )
                            .on_click(cx.listener(set(ZoomOptions {
                                show_background: !options.show_background,
                                ..options
                            }))),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .pt_1()
                        .child(
                            div()
                                .w(px(80.0))
                                .text_xs()
                                .text_color(theme::text_dim())
                                .child(i18n::text("anim.duration")),
                        )
                        .child(
                            segmented("zoom-duration", 0, "−", false).on_click(cx.listener(set(
                                ZoomOptions {
                                    transition_ms: options.transition_ms.saturating_sub(250),
                                    ..options
                                },
                            ))),
                        )
                        .child(
                            div()
                                .w(px(56.0))
                                .text_xs()
                                .text_center()
                                .child(format!("{:.2}", f64::from(options.transition_ms) / 1000.0)),
                        )
                        .child(
                            segmented("zoom-duration", 1, "+", false).on_click(cx.listener(set(
                                ZoomOptions {
                                    transition_ms: options.transition_ms + 250,
                                    ..options
                                },
                            ))),
                        ),
                );
        }
        body
    }

    /// 挿入: zooms to the ticked slides or sections.
    fn insert_zooms(&mut self, cx: &mut Context<Self>) {
        let kind = self.anim.zoom_kind;
        let picks = std::mem::take(&mut self.anim.picks);
        let targets: Vec<ZoomTarget> = match kind {
            ZoomKind::Slide => picks
                .iter()
                .filter_map(|&index| self.deck.slide_id(index))
                .map(ZoomTarget::Slide)
                .collect(),
            _ => {
                let sections = self.deck.sections();
                picks
                    .iter()
                    .filter_map(|&index| sections.get(index))
                    .map(|section| ZoomTarget::Section(section.id.clone()))
                    .collect()
            }
        };
        if targets.is_empty() {
            cx.notify();
            return;
        }
        let slide = self.current;
        match self.deck.insert_zoom(slide, kind, &targets, (48.0, 48.0)) {
            Ok(ids) => {
                self.reload(false, cx);
                self.selected = ids.first().copied();
            }
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]));
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jade_office_model::deck::animation::AnimationTarget;

    fn effect(start: EffectStart, delay: u32, duration: u32) -> PaneEffect {
        let mut effect = jade_office_model::deck::animation::AnimationEffect {
            class: EffectClass::Entrance,
            preset_id: 10,
            preset_subtype: 0,
            start,
            target: AnimationTarget::shape(2),
            delay_ms: delay,
            duration_ms: duration,
            group_id: None,
            accel: 0.0,
            decel: 0.0,
            auto_reverse: false,
            repeat: Repeat::None,
            rewind: false,
            after: AfterEffect::None,
            iterate: None,
            behaviors: Vec::new(),
            node: Default::default(),
        };
        effect.start = start;
        PaneEffect {
            effect,
            trigger: None,
        }
    }

    #[gpui::test]
    fn the_panes_add_effects_set_transitions_and_insert_zooms(cx: &mut gpui::TestAppContext) {
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/public/fixtures/sample-deck.pptx"
        ));
        let (view, cx) = cx.add_window_view(|_window, cx| DeckView::open(&path, cx).unwrap());
        cx.run_until_parked();
        let shape = view.read_with(cx, |view, _| view.shapes[0].id);
        let fade = PRESETS
            .iter()
            .find(|p| p.class == PresetClass::Entrance && p.name == "fade")
            .unwrap();
        let spin = PRESETS
            .iter()
            .find(|p| p.class == PresetClass::Emphasis && p.name == "spin")
            .unwrap();
        view.update(cx, |view, cx| {
            view.anim.tab = Some(Tab::Animations);
            view.selected = Some(shape);
            view.choose_effect(fade, cx);
            // アニメーションの追加: a second effect on the same shape.
            view.anim.adding = true;
            view.anim.selected.clear();
            view.choose_effect(spin, cx);
        });
        cx.run_until_parked();
        let pane = view.read_with(cx, |view, _| view.deck.animation_pane(0));
        assert_eq!(pane.len(), 2);
        assert_eq!(pane[0].effect.preset_id, 10);
        assert_eq!(pane[1].effect.class, EffectClass::Emphasis);

        view.update(cx, |view, cx| {
            view.anim.selected = vec![1];
            view.edit_anim(
                AnimationEdit::SetStart {
                    indices: vec![1],
                    start: EffectStart::AfterPrevious,
                },
                cx,
            );
            view.anim.tab = Some(Tab::Transitions);
            let push = TRANSITIONS.iter().find(|t| t.key == "push").unwrap();
            let effect = jade_office_model::deck::animation::preset_effect(push, None);
            view.edit_transition(vec![0], TransitionEdit::Effect(Some(effect)), cx);
            let all = (0..view.deck.slide_count()).collect();
            let transition = view.deck.transition(0).unwrap();
            view.edit_transition(all, TransitionEdit::Replace(transition), cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            let times = timeline(&view.deck.animation_pane(0));
            assert_eq!(times[1].0, 1, "after previous stays in the first click");
            assert!(times[1].1 >= times[0].2);
            for slide in 0..view.deck.slide_count() {
                let effect = view.deck.transition(slide).and_then(|t| t.effect);
                assert_eq!(effect.map(|e| e.name), Some("push".to_string()));
            }
        });

        view.update(cx, |view, cx| {
            view.anim.tab = Some(Tab::Zoom);
            view.anim.zoom_kind = ZoomKind::Slide;
            view.anim.picks = vec![1];
            view.insert_zooms(cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            let zooms = view.deck.zooms(0);
            assert_eq!(zooms.len(), 1);
            assert_eq!(zooms[0].slide_id, view.deck.slide_id(1));
        });
    }

    #[test]
    fn the_timeline_numbers_clicks_and_follows_steps() {
        let pane = vec![
            effect(EffectStart::OnClick, 0, 500),
            effect(EffectStart::WithPrevious, 250, 500),
            effect(EffectStart::AfterPrevious, 0, 1000),
            effect(EffectStart::OnClick, 0, 500),
        ];
        let times = timeline(&pane);
        assert_eq!(times[0], (1, 0.0, 500.0));
        assert_eq!(times[1], (1, 250.0, 750.0));
        assert_eq!(times[2], (1, 750.0, 1750.0));
        assert_eq!(times[3], (2, 0.0, 500.0));
        assert_eq!(effect_name(&pane[0].effect), i18n::text("anim.effect.fade"));
    }
}

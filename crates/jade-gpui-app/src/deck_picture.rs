//! Pictures in the presentation view, as PowerPoint for Mac's 図の形式 tab
//! shows them while a picture is selected: 図の変更, 修整, 色, 透明度,
//! アート効果, 背景の削除, 図のスタイル, 図の枠線, 図の効果, トリミング
//! (縦横比, 図形に合わせて), 回転, サイズ with 縦横比を固定, 図のリセット
//! and 図の圧縮, plus グラフィックの塗りつぶし on an SVG. On the 挿入 side:
//! アイコン, スクリーンショット and フォト アルバム. Animated GIFs play.
//!
//! A child module of the view (`deck.rs` names it), so it reaches the
//! view's state; the editing itself is jade-office-model's.

use std::time::Duration;

use gpui::{div, prelude::*, px, svg, AnyElement, Context, Div, SharedString, Stateful};
use jade_office_model::deck::{
    recolor_presets, PictureEdit, PictureEffect, ARTISTIC_EFFECTS, COMPRESS_RESOLUTIONS,
    CROP_ASPECTS, ICONS, PICTURE_STYLES,
};

use super::{menu_panel, menu_row, with_menu, DeckView};
use crate::{i18n, theme};

/// A list open from the picture tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicturePopup {
    Corrections,
    Color,
    Transparency,
    Artistic,
    Styles,
    Border,
    Effects,
    Crop,
    Rotate,
    Size,
    Reset,
    Compress,
    GraphicFill,
    Icons,
    Album,
}

/// The picture tools' state.
pub struct PictureTools {
    pub popup: Option<PicturePopup>,
    /// 図の圧縮's 図のトリミング部分を削除する.
    pub delete_cropped: bool,
    /// An animated GIF on the slide is being played.
    pub playing: bool,
    pub started: std::time::Instant,
    /// The picture in トリミング, its crop handles shown.
    pub cropping: Option<u32>,
    /// A crop handle held.
    pub crop_drag: Option<super::picture_crop::CropDrag>,
}

impl Default for PictureTools {
    fn default() -> Self {
        Self {
            popup: None,
            delete_cropped: true,
            playing: false,
            started: std::time::Instant::now(),
            cropping: None,
            crop_drag: None,
        }
    }
}

fn japanese() -> bool {
    i18n::language() == i18n::Language::Japanese
}

const SWATCHES: [&str; 10] = [
    "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47", "FF0000", "000000", "FFFFFF",
    "7030A0",
];

const CROP_SHAPES: [(&str, &str, &str); 8] = [
    ("rect", "四角形", "Rectangle"),
    ("roundRect", "四角形: 角を丸くする", "Rounded Rectangle"),
    ("ellipse", "楕円", "Oval"),
    ("triangle", "二等辺三角形", "Triangle"),
    ("diamond", "ひし形", "Diamond"),
    ("hexagon", "六角形", "Hexagon"),
    ("star5", "星: 5 pt", "5-Point Star"),
    ("heart", "ハート", "Heart"),
];

fn bar_button(id: &'static str, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .px_2()
        .py_0p5()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .text_color(theme::text_dim())
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()).text_color(theme::text()))
        .child(label)
}

fn swatch(color: &str) -> Div {
    let value = u32::from_str_radix(color, 16).unwrap_or(0);
    div()
        .size_3()
        .rounded_sm()
        .border_1()
        .border_color(theme::border())
        .bg(gpui::rgb(value))
}

fn heading(label: &'static str) -> Div {
    div()
        .px_1()
        .pt_1()
        .text_xs()
        .text_color(theme::text_dim())
        .child(label)
}

fn row_id(prefix: &str, value: impl std::fmt::Display) -> SharedString {
    SharedString::from(format!("picture-{prefix}-{value}"))
}

impl DeckView {
    /// The picture selected on its own, if the selection is one.
    pub(super) fn selected_picture(&self) -> Option<u32> {
        let id = self.selected.filter(|_| self.more.is_empty())?;
        (self.shape(id)?.kind == "picture").then_some(id)
    }

    fn picture_popup(&mut self, popup: PicturePopup, cx: &mut Context<Self>) {
        self.pictures.popup = if self.pictures.popup == Some(popup) {
            None
        } else {
            Some(popup)
        };
        self.popup = None;
        cx.notify();
    }

    fn picture_done(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        self.pictures.popup = None;
        match result {
            Ok(_) => self.reload(true, cx),
            Err(error) => {
                self.status =
                    SharedString::from(i18n::format("deck.edit_failed", &[("error", &error)]))
            }
        }
        cx.notify();
    }

    /// One 図の形式 command on the selected picture.
    pub(super) fn format_picture(&mut self, edit: PictureEdit, cx: &mut Context<Self>) {
        let Some(id) = self.selected_picture() else {
            return;
        };
        let result = self.deck.format_picture(self.current, id, &edit);
        self.picture_done(result, cx);
    }

    /// 図形に変換: the selected SVG picture becomes a group of freeforms,
    /// selected in its place.
    pub(super) fn convert_picture_to_shapes(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_picture() else {
            return;
        };
        match self.deck.convert_svg_to_shapes(self.current, id) {
            Ok(group) => {
                self.picture_done(Ok(true), cx);
                self.selected = Some(group);
            }
            Err(error) => self.picture_done(Err(error), cx),
        }
    }

    pub(super) fn picture_command(
        &mut self,
        command: impl FnOnce(&mut Self, usize, u32) -> Result<bool, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = self.selected_picture() else {
            return;
        };
        let current = self.current;
        let result = command(self, current, id);
        self.picture_done(result, cx);
    }

    /// Asks for picture files, then hands their bytes to `then`.
    fn pick_pictures(
        &mut self,
        multiple: bool,
        then: impl FnOnce(&mut Self, Vec<Vec<u8>>, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        self.pictures.popup = None;
        let prompt = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let pictures: Vec<Vec<u8>> = paths
                .iter()
                .filter_map(|path| std::fs::read(path).ok())
                .collect();
            if pictures.is_empty() {
                return;
            }
            view.update(cx, |view, cx| then(view, pictures, cx)).ok();
        })
        .detach();
    }

    /// 図の変更 › ファイルから.
    fn change_picture(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected_picture() else {
            return;
        };
        self.pick_pictures(
            false,
            move |view, pictures, cx| {
                let result = view.deck.change_picture(view.current, id, &pictures[0]);
                view.picture_done(result, cx);
            },
            cx,
        );
    }

    /// 挿入 › フォト アルバム with `per_slide` pictures on each new slide.
    pub(super) fn insert_photo_album(&mut self, per_slide: usize, cx: &mut Context<Self>) {
        self.pick_pictures(
            true,
            move |view, pictures, cx| {
                let after = view.current;
                match view.deck.insert_photo_album(after, &pictures, per_slide) {
                    Ok(first) => {
                        view.reload(true, cx);
                        view.go_to(first, cx);
                    }
                    Err(error) => {
                        view.status = SharedString::from(i18n::format(
                            "deck.picture_failed",
                            &[("error", &error)],
                        ));
                        cx.notify();
                    }
                }
            },
            cx,
        );
    }

    /// 挿入 › スクリーンショット: the area the user picks, as macOS's
    /// screen capture takes it.
    pub(super) fn insert_screenshot(&mut self, cx: &mut Context<Self>) {
        self.pictures.popup = None;
        if !cfg!(target_os = "macos") {
            self.status = SharedString::from(i18n::text("deck.picture.screenshot_unsupported"));
            cx.notify();
            return;
        }
        let path = std::env::temp_dir().join(format!("jade-screenshot-{}.png", std::process::id()));
        let task = cx.background_executor().spawn({
            let path = path.clone();
            async move {
                let status = std::process::Command::new("/usr/sbin/screencapture")
                    .args(["-i", "-x"])
                    .arg(&path)
                    .status();
                let bytes = status
                    .ok()
                    .filter(|s| s.success())
                    .and_then(|_| std::fs::read(&path).ok());
                let _ = std::fs::remove_file(&path);
                bytes
            }
        });
        cx.spawn(async move |view, cx| {
            let Some(bytes) = task.await else {
                return;
            };
            view.update(cx, |view, cx| {
                let result = view.deck.insert_picture(view.current, &bytes);
                view.inserted(result, cx);
            })
            .ok();
        })
        .detach();
    }

    /// The slide drawn again: an animated GIF has moved on.
    fn repaint_slide(&mut self, cx: &mut Context<Self>) {
        if let Some(painted) = self.slide.take() {
            self.retired.push(painted.image);
        }
        cx.notify();
    }

    /// 挿入 › アイコン › one icon.
    fn insert_icon(&mut self, index: usize, cx: &mut Context<Self>) {
        self.pictures.popup = None;
        let result = self.deck.insert_icon(self.current, index);
        self.inserted(result, cx);
    }

    /// Plays the slide's animated GIFs: the slide redrawn ten times a
    /// second while one is on it.
    pub(super) fn play_pictures(&mut self, cx: &mut Context<Self>) {
        if self.pictures.playing || !self.deck.has_animated_pictures(self.current) {
            return;
        }
        self.pictures.playing = true;
        cx.spawn(async move |view, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let keep = view
                .update(cx, |view, cx| {
                    if !view.deck.has_animated_pictures(view.current) {
                        view.pictures.playing = false;
                        return false;
                    }
                    let elapsed = view.pictures.started.elapsed().as_millis() as u64;
                    jade_office_model::deck::set_picture_clock(elapsed);
                    view.repaint_slide(cx);
                    true
                })
                .unwrap_or(false);
            if !keep {
                break;
            }
        })
        .detach();
    }

    fn picture_list(
        &self,
        id: &'static str,
        label: &'static str,
        popup: PicturePopup,
        rows: impl FnOnce(&mut Context<Self>) -> Vec<AnyElement>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.pictures.popup == Some(popup);
        let menu = open.then(|| menu_panel(id).children(rows(cx)).into_any_element());
        with_menu(
            bar_button(id, label)
                .child(
                    svg()
                        .path("icons/chevron-down.svg")
                        .size_3()
                        .ml_0p5()
                        .text_color(theme::text_dim()),
                )
                .when(open, |button| button.bg(theme::selected()))
                .on_click(
                    cx.listener(move |view, _event, _window, cx| view.picture_popup(popup, cx)),
                )
                .into_any_element(),
            menu,
        )
    }

    fn edit_row(
        &self,
        id: SharedString,
        label: impl Into<SharedString>,
        edit: PictureEdit,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        menu_row(id, label, false)
            .on_click(
                cx.listener(move |view, _event, _window, cx| view.format_picture(edit.clone(), cx)),
            )
            .into_any_element()
    }

    /// 挿入's picture buttons: アイコン, スクリーンショット, フォト アルバム.
    pub(super) fn picture_insert_buttons(&self, cx: &mut Context<Self>) -> AnyElement {
        let icons =
            self.picture_list(
                "picture-icons",
                i18n::text("deck.picture.icons"),
                PicturePopup::Icons,
                |cx| {
                    ICONS
                        .iter()
                        .enumerate()
                        .map(|(index, (ja, en, _))| {
                            menu_row(
                                row_id("icon", index),
                                if japanese() { *ja } else { *en },
                                false,
                            )
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.insert_icon(index, cx)
                            }))
                            .into_any_element()
                        })
                        .collect()
                },
                cx,
            );
        let album = self.picture_list(
            "picture-album",
            i18n::text("deck.picture.photo_album"),
            PicturePopup::Album,
            |cx| {
                [1usize, 2, 4]
                    .into_iter()
                    .map(|count| {
                        menu_row(
                            row_id("album", count),
                            i18n::format("deck.picture.album_layout", &[("count", &count)]),
                            false,
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            view.insert_photo_album(count, cx)
                        }))
                        .into_any_element()
                    })
                    .collect()
            },
            cx,
        );
        div()
            .flex()
            .items_center()
            .child(icons)
            .child(
                bar_button("picture-screenshot", i18n::text("deck.picture.screenshot"))
                    .on_click(cx.listener(|view, _event, _window, cx| view.insert_screenshot(cx))),
            )
            .child(album)
            .into_any_element()
    }

    /// The 図の形式 tab's controls, while a picture is selected.
    pub(super) fn picture_bar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.selected_picture()?;
        let info = self.deck.picture_format(self.current, id)?;
        let rect = self.deck.shape_rect(self.current, id)?;
        let corrections = self.picture_list(
            "picture-corrections",
            i18n::text("deck.picture.corrections"),
            PicturePopup::Corrections,
            |cx| {
                let mut rows =
                    vec![heading(i18n::text("deck.picture.sharpness")).into_any_element()];
                for amount in [-0.5, -0.25, 0.0, 0.25, 0.5] {
                    rows.push(self.edit_row(
                        row_id("sharpen", amount),
                        format!("{:+}%", (amount * 100.0) as i32),
                        PictureEdit::Sharpness(amount),
                        cx,
                    ));
                }
                rows.push(
                    heading(i18n::text("deck.picture.brightness_contrast")).into_any_element(),
                );
                for brightness in [-0.4, -0.2, 0.0, 0.2, 0.4] {
                    for contrast in [-0.4, 0.0, 0.4] {
                        rows.push(self.edit_row(
                            row_id("bc", format!("{brightness}-{contrast}")),
                            format!(
                                "{} {:+}% / {} {:+}%",
                                i18n::text("deck.picture.brightness"),
                                (brightness * 100.0) as i32,
                                i18n::text("deck.picture.contrast"),
                                (contrast * 100.0) as i32
                            ),
                            PictureEdit::Corrections {
                                brightness,
                                contrast,
                            },
                            cx,
                        ));
                    }
                }
                rows
            },
            cx,
        );
        let color = self.picture_list(
            "picture-color",
            i18n::text("deck.picture.color"),
            PicturePopup::Color,
            |cx| {
                let mut rows =
                    vec![heading(i18n::text("deck.picture.saturation")).into_any_element()];
                for amount in [0.0, 0.33, 0.66, 1.0, 1.33, 1.66, 2.0, 3.0, 4.0] {
                    rows.push(self.edit_row(
                        row_id("saturation", amount),
                        format!("{}%", (amount * 100.0_f64).round() as i32),
                        PictureEdit::Saturation(amount),
                        cx,
                    ));
                }
                rows.push(heading(i18n::text("deck.picture.tone")).into_any_element());
                for kelvin in [
                    4700.0, 5300.0, 5900.0, 6500.0, 7200.0, 7800.0, 8800.0, 10000.0, 11200.0,
                ] {
                    rows.push(self.edit_row(
                        row_id("tone", kelvin),
                        format!("{kelvin} K"),
                        PictureEdit::Temperature(kelvin),
                        cx,
                    ));
                }
                rows.push(heading(i18n::text("deck.picture.recolor")).into_any_element());
                for (index, (ja, en, recolor)) in recolor_presets().into_iter().enumerate() {
                    rows.push(self.edit_row(
                        row_id("recolor", index),
                        if japanese() { ja } else { en },
                        PictureEdit::Recolor(recolor),
                        cx,
                    ));
                }
                rows.push(heading(i18n::text("deck.picture.set_transparent")).into_any_element());
                rows.push(self.edit_row(
                    row_id("transparent", "white"),
                    i18n::text("deck.picture.transparent_white"),
                    PictureEdit::TransparentColor(Some("FFFFFF".into())),
                    cx,
                ));
                rows.push(self.edit_row(
                    row_id("transparent", "black"),
                    i18n::text("deck.picture.transparent_black"),
                    PictureEdit::TransparentColor(Some("000000".into())),
                    cx,
                ));
                rows.push(self.edit_row(
                    row_id("transparent", "none"),
                    i18n::text("deck.picture.none"),
                    PictureEdit::TransparentColor(None),
                    cx,
                ));
                rows
            },
            cx,
        );
        let current_transparency = info.transparency;
        let transparency = self.picture_list(
            "picture-transparency",
            i18n::text("deck.picture.transparency"),
            PicturePopup::Transparency,
            |cx| {
                [0.0, 0.15, 0.3, 0.5, 0.65, 0.8, 0.95]
                    .into_iter()
                    .map(|amount: f64| {
                        menu_row(
                            row_id("transparency", amount),
                            format!("{}%", (amount * 100.0).round() as i32),
                            (current_transparency - amount).abs() < 0.01,
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            view.format_picture(PictureEdit::Transparency(amount), cx)
                        }))
                        .into_any_element()
                    })
                    .collect()
            },
            cx,
        );
        let artistic = self.picture_list(
            "picture-artistic",
            i18n::text("deck.picture.artistic"),
            PicturePopup::Artistic,
            |cx| {
                let mut rows = vec![self.edit_row(
                    row_id("artistic", "none"),
                    i18n::text("deck.picture.none"),
                    PictureEdit::Artistic(None),
                    cx,
                )];
                for (element, ja, en) in ARTISTIC_EFFECTS {
                    rows.push(self.edit_row(
                        row_id("artistic", element),
                        if japanese() { *ja } else { *en },
                        PictureEdit::Artistic(Some(element.to_string())),
                        cx,
                    ));
                }
                rows
            },
            cx,
        );
        let styles = self.picture_list(
            "picture-styles",
            i18n::text("deck.picture.styles"),
            PicturePopup::Styles,
            |cx| {
                PICTURE_STYLES
                    .iter()
                    .enumerate()
                    .map(|(index, style)| {
                        self.edit_row(
                            row_id("style", index),
                            if japanese() { style.ja } else { style.en },
                            PictureEdit::Style(index),
                            cx,
                        )
                    })
                    .collect()
            },
            cx,
        );
        let border = self.picture_list(
            "picture-border",
            i18n::text("deck.picture.border"),
            PicturePopup::Border,
            |cx| {
                let mut rows = vec![self.edit_row(
                    row_id("border", "none"),
                    i18n::text("deck.picture.no_border"),
                    PictureEdit::Border {
                        color: None,
                        width_pt: None,
                    },
                    cx,
                )];
                for color in SWATCHES {
                    rows.push(
                        menu_row(row_id("border", color), format!("#{color}"), false)
                            .child(swatch(color))
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.format_picture(
                                    PictureEdit::Border {
                                        color: Some(color.to_string()),
                                        width_pt: None,
                                    },
                                    cx,
                                )
                            }))
                            .into_any_element(),
                    );
                }
                rows.push(heading(i18n::text("deck.picture.weight")).into_any_element());
                for weight in [0.75, 1.5, 3.0, 6.0] {
                    rows.push(self.edit_row(
                        row_id("weight", weight),
                        format!("{weight} pt"),
                        PictureEdit::Border {
                            color: Some("000000".into()),
                            width_pt: Some(weight),
                        },
                        cx,
                    ));
                }
                rows
            },
            cx,
        );
        let effects = self.picture_list(
            "picture-effects",
            i18n::text("deck.picture.effects"),
            PicturePopup::Effects,
            |cx| {
                let toggles: [(&'static str, PictureEffect, PictureEffect); 5] = [
                    (
                        "deck.picture.shadow",
                        PictureEffect::Shadow(true),
                        PictureEffect::Shadow(false),
                    ),
                    (
                        "deck.picture.reflection",
                        PictureEffect::Reflection(true),
                        PictureEffect::Reflection(false),
                    ),
                    (
                        "deck.picture.glow",
                        PictureEffect::Glow(Some(("FFC000".into(), 10.0))),
                        PictureEffect::Glow(None),
                    ),
                    (
                        "deck.picture.soft_edges",
                        PictureEffect::SoftEdge(Some(10.0)),
                        PictureEffect::SoftEdge(None),
                    ),
                    (
                        "deck.picture.bevel",
                        PictureEffect::Bevel(true),
                        PictureEffect::Bevel(false),
                    ),
                ];
                let mut rows = Vec::new();
                for (key, on, off) in toggles {
                    rows.push(heading(i18n::text(key)).into_any_element());
                    rows.push(self.edit_row(
                        row_id("effect-on", key),
                        i18n::text(key),
                        PictureEdit::Effect(on),
                        cx,
                    ));
                    rows.push(self.edit_row(
                        row_id("effect-off", key),
                        i18n::text("deck.picture.none"),
                        PictureEdit::Effect(off),
                        cx,
                    ));
                }
                rows
            },
            cx,
        );
        let cropping = self.cropping_picture().is_some();
        let crop = self.picture_list(
            "picture-crop",
            i18n::text("deck.picture.crop"),
            PicturePopup::Crop,
            |cx| {
                let mut rows = vec![menu_row(
                    "picture-crop-handles",
                    i18n::text("deck.picture.crop_handles"),
                    cropping,
                )
                .on_click(cx.listener(|view, _event, _window, cx| view.toggle_crop_handles(cx)))
                .into_any_element()];
                for (side, key) in [
                    (0, "deck.picture.crop_left"),
                    (1, "deck.picture.crop_top"),
                    (2, "deck.picture.crop_right"),
                    (3, "deck.picture.crop_bottom"),
                ] {
                    rows.push(
                        menu_row(
                            row_id("crop-side", side),
                            format!("{} 5%", i18n::text(key)),
                            false,
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            view.picture_command(
                                move |view, slide, id| {
                                    let mut crop =
                                        view.deck.picture_crop(slide, id).unwrap_or_default();
                                    match side {
                                        0 => crop.left += 5_000,
                                        1 => crop.top += 5_000,
                                        2 => crop.right += 5_000,
                                        _ => crop.bottom += 5_000,
                                    }
                                    view.deck.crop_picture(slide, id, crop)
                                },
                                cx,
                            )
                        }))
                        .into_any_element(),
                    );
                }
                rows.push(
                    menu_row("picture-uncrop", i18n::text("deck.picture.uncrop"), false)
                        .on_click(cx.listener(|view, _event, _window, cx| {
                            view.picture_command(
                                |view, slide, id| {
                                    view.deck.crop_picture(slide, id, Default::default())
                                },
                                cx,
                            )
                        }))
                        .into_any_element(),
                );
                rows.push(heading(i18n::text("deck.picture.crop_to_shape")).into_any_element());
                for (preset, ja, en) in CROP_SHAPES {
                    rows.push(self.edit_row(
                        row_id("crop-shape", preset),
                        if japanese() { ja } else { en },
                        PictureEdit::CropToShape(preset.to_string()),
                        cx,
                    ));
                }
                rows.push(heading(i18n::text("deck.picture.aspect")).into_any_element());
                for (label, w, h) in CROP_ASPECTS {
                    let (w, h) = (*w, *h);
                    rows.push(
                        menu_row(row_id("aspect", label), *label, false)
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.picture_command(
                                    move |view, slide, id| {
                                        view.deck.crop_picture_to_aspect(slide, id, w, h)
                                    },
                                    cx,
                                )
                            }))
                            .into_any_element(),
                    );
                }
                rows
            },
            cx,
        );
        let rotate = self.picture_list(
            "picture-rotate",
            i18n::text("deck.picture.rotate"),
            PicturePopup::Rotate,
            |cx| {
                let commands: [(&'static str, u8); 4] = [
                    ("deck.picture.rotate_right", 0),
                    ("deck.picture.rotate_left", 1),
                    ("deck.picture.flip_vertical", 2),
                    ("deck.picture.flip_horizontal", 3),
                ];
                commands
                    .into_iter()
                    .map(|(key, which)| {
                        menu_row(row_id("rotate", which), i18n::text(key), false)
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                view.picture_command(
                                    move |view, slide, id| match which {
                                        0 => view.deck.rotate_picture(slide, id, 90.0),
                                        1 => view.deck.rotate_picture(slide, id, -90.0),
                                        2 => view.deck.flip_picture(slide, id, false),
                                        _ => view.deck.flip_picture(slide, id, true),
                                    },
                                    cx,
                                )
                            }))
                            .into_any_element()
                    })
                    .collect()
            },
            cx,
        );
        let lock = info.lock_aspect;
        let size = self.picture_list(
            "picture-size",
            i18n::text("deck.picture.size"),
            PicturePopup::Size,
            |cx| {
                // 0.1 inch steps, as PowerPoint's size boxes step.
                let step = 9.6;
                let mut rows = Vec::new();
                for (key, height) in [("deck.picture.height", true), ("deck.picture.width", false)]
                {
                    let value = if height { rect.height } else { rect.width };
                    rows.push(
                        heading(i18n::text(key))
                            .child(format!(" {:.2} cm", value / 96.0 * 2.54))
                            .into_any_element(),
                    );
                    for (sign, label) in [(1.0, "+0.1\""), (-1.0, "−0.1\"")] {
                        rows.push(
                            menu_row(
                                row_id(if height { "height" } else { "width" }, label),
                                label,
                                false,
                            )
                            .on_click(cx.listener(move |view, _event, _window, cx| {
                                let next = (value + sign * step).max(1.0);
                                view.picture_command(
                                    move |view, slide, id| {
                                        if height {
                                            view.deck.set_picture_size(slide, id, None, Some(next))
                                        } else {
                                            view.deck.set_picture_size(slide, id, Some(next), None)
                                        }
                                    },
                                    cx,
                                )
                            }))
                            .into_any_element(),
                        );
                    }
                }
                rows.push(
                    menu_row(
                        "picture-lock-aspect",
                        i18n::text("deck.picture.lock_aspect"),
                        lock,
                    )
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.format_picture(PictureEdit::LockAspect(!lock), cx)
                    }))
                    .into_any_element(),
                );
                rows
            },
            cx,
        );
        let reset = self.picture_list(
            "picture-reset",
            i18n::text("deck.picture.reset"),
            PicturePopup::Reset,
            |cx| {
                vec![
                    self.edit_row(
                        "picture-reset-format".into(),
                        i18n::text("deck.picture.reset"),
                        PictureEdit::Reset,
                        cx,
                    ),
                    self.edit_row(
                        "picture-reset-size".into(),
                        i18n::text("deck.picture.reset_size"),
                        PictureEdit::ResetWithSize,
                        cx,
                    ),
                ]
            },
            cx,
        );
        let delete_cropped = self.pictures.delete_cropped;
        let compress = self.picture_list(
            "picture-compress",
            i18n::text("deck.picture.compress"),
            PicturePopup::Compress,
            |cx| {
                let mut rows = vec![menu_row(
                    "picture-delete-cropped",
                    i18n::text("deck.picture.delete_cropped"),
                    delete_cropped,
                )
                .on_click(cx.listener(|view, _event, _window, cx| {
                    view.pictures.delete_cropped = !view.pictures.delete_cropped;
                    cx.notify();
                }))
                .into_any_element()];
                for (ja, en, ppi) in COMPRESS_RESOLUTIONS {
                    let ppi = *ppi;
                    rows.push(
                        menu_row(
                            row_id("compress", ppi),
                            if japanese() { *ja } else { *en },
                            false,
                        )
                        .on_click(cx.listener(move |view, _event, _window, cx| {
                            let Some(id) = view.selected_picture() else {
                                return;
                            };
                            let only = Some((view.current, id));
                            let delete = view.pictures.delete_cropped;
                            match view.deck.compress_pictures(only, ppi, delete) {
                                Ok(count) => {
                                    view.status = SharedString::from(i18n::format(
                                        "deck.picture.compressed",
                                        &[("count", &count)],
                                    ));
                                    view.picture_done(Ok(count > 0), cx);
                                }
                                Err(error) => view.picture_done(Err(error), cx),
                            }
                        }))
                        .into_any_element(),
                    );
                }
                rows
            },
            cx,
        );
        let graphic_fill = info.svg.then(|| {
            self.picture_list(
                "picture-graphic-fill",
                i18n::text("deck.picture.graphic_fill"),
                PicturePopup::GraphicFill,
                |cx| {
                    SWATCHES
                        .iter()
                        .map(|color| {
                            menu_row(row_id("graphic-fill", color), format!("#{color}"), false)
                                .child(swatch(color))
                                .on_click(cx.listener(move |view, _event, _window, cx| {
                                    view.format_picture(
                                        PictureEdit::GraphicFill(color.to_string()),
                                        cx,
                                    )
                                }))
                                .into_any_element()
                        })
                        .collect()
                },
                cx,
            )
        });
        let background_removed = self.deck.picture_has_background_removed(self.current, id);
        Some(
            div()
                .id("picture-bar")
                .flex()
                .flex_wrap()
                .items_center()
                .gap_0p5()
                .px_2()
                .py_1()
                .border_b_1()
                .border_color(theme::border())
                .child(
                    div()
                        .px_1()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme::text())
                        .child(i18n::text("deck.picture.format")),
                )
                .child(
                    bar_button("picture-change", i18n::text("deck.picture.change"))
                        .on_click(cx.listener(|view, _event, _window, cx| view.change_picture(cx))),
                )
                .child(
                    bar_button(
                        "picture-remove-background",
                        i18n::text(if background_removed {
                            "deck.picture.keep_background"
                        } else {
                            "deck.picture.remove_background"
                        }),
                    )
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        let marquee = (!background_removed).then_some([0.0, 0.0, 1.0, 1.0]);
                        view.format_picture(PictureEdit::RemoveBackground(marquee), cx)
                    })),
                )
                .child(corrections)
                .child(color)
                .child(artistic)
                .child(transparency)
                .child(div().w(px(1.0)).h(px(18.0)).mx_1().bg(theme::border()))
                .child(styles)
                .child(border)
                .child(effects)
                .children(graphic_fill)
                .children(info.svg.then(|| {
                    bar_button(
                        "picture-convert-shapes",
                        i18n::text("deck.picture.convert_to_shapes"),
                    )
                    .on_click(
                        cx.listener(|view, _event, _window, cx| view.convert_picture_to_shapes(cx)),
                    )
                }))
                .child(div().w(px(1.0)).h(px(18.0)).mx_1().bg(theme::border()))
                .child(crop)
                .child(rotate)
                .child(size)
                .child(reset)
                .child(compress)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use gpui::{Entity, Focusable};
    use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
    use jade_office_model::deck::{PictureEdit, Recolor};

    use super::super::DeckView;

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

    fn png() -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 32, Rgb([200, 30, 30])))
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    /// A picture inserted is selected with the 図の形式 controls; 色の変更,
    /// 回転, 縦横比を固定 and 図のリセット act on it, each one Undo.
    #[gpui::test]
    fn the_picture_format_tab_edits_the_selected_picture(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        view.update(cx, |view, cx| view.insert_picture(&png(), cx));
        cx.run_until_parked();
        let id = view.read_with(cx, |view, _| {
            view.selected_picture().expect("the picture is selected")
        });
        view.update(cx, |view, cx| {
            assert!(view.picture_bar(cx).is_some(), "the 図の形式 controls show");
            view.format_picture(PictureEdit::Recolor(Recolor::Grayscale), cx);
            view.picture_command(
                |view, slide, id| view.deck.rotate_picture(slide, id, 90.0),
                cx,
            );
            view.format_picture(PictureEdit::LockAspect(false), cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            let info = view.deck.picture_format(view.current, id).unwrap();
            assert!(!info.lock_aspect);
            assert_eq!(
                view.shape(id).unwrap().transform.unwrap().rotation_60000ths,
                90 * 60_000
            );
        });
        view.update(cx, |view, cx| {
            view.format_picture(PictureEdit::ResetWithSize, cx)
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            let rect = view.shape(id).unwrap().transform.unwrap();
            assert_eq!(rect.extent_emu, (64 * 9525, 32 * 9525));
        });
        // An icon from 挿入 › アイコン is an SVG picture with a graphic fill.
        view.update(cx, |view, cx| view.insert_icon(1, cx));
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            let icon = view.selected_picture().expect("the icon is selected");
            assert!(view.deck.picture_format(view.current, icon).unwrap().svg);
        });
        // 図形に変換 leaves the group of freeforms selected.
        view.update(cx, |view, cx| view.convert_picture_to_shapes(cx));
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(view.selected_picture().is_none());
            assert!(view.selected.is_some());
        });
    }
}

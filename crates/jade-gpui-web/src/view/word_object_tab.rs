//! The contextual tab Word shows in the browser while a drawing object is
//! selected — 図の形式 for a picture, 図形の書式 for a shape, text box or
//! WordArt — with its 調整 (修整, 色, アート効果, 透明度, 図の変更, リセット,
//! 背景の削除), スタイル (枠線, 効果; a shape's style gallery and 塗りつぶし),
//! ワードアート (文字の塗りつぶし, 変形, 文字の配置), 配置 (位置, 折り返し, 前面へ
//! / 背面へ, 配置, グループ化, 回転, the Selection Pane) and サイズ (トリミング,
//! the aspect ratios and shapes, the height and width) groups: the native
//! app's `word_objects_ribbon`, each control one of `jade-office-model`'s
//! object commands.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_web::word::objects::{
    Align, AlignTo, Arrange, ObjectFormat, ObjectSort, Rotate, WordObjects, PX_PER_MM,
};
use jade_web::word::picture::{Wrap, WRAPS};
use jade_web::word::picture_format::{
    Dash, Effect, OutlineEdit, Recolor, ReflectionPreset, ShadowPreset, WordPictureFormat,
    ARTISTIC_EFFECTS, ASPECT_RATIOS, CORRECTION_STEPS, SATURATIONS, TEMPERATURES,
};
use jade_web::word::shape_gallery::{crop_shapes, CATEGORIES};
use jade_web::word::shapes::{
    Fill, Paint, ShapeStyle, TextBoxShape, WordShapes, STYLE_COLORS, TEXT_WARPS,
};

use super::word_objects_ui::ObjectUi;
use super::word_tools_ui::Pick;
use super::{DocView, Document};
use crate::i18n::{self, text as t};
use crate::palette as theme;
use crate::ribbon::{
    dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup,
};
use crate::tooltip::Tooltip;

/// The tab's lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormatPopup {
    ChangeShape,
    Corrections,
    Color,
    Artistic,
    Transparency,
    ChangePicture,
    Reset,
    Outline,
    Effects,
    Position,
    Wrap,
    Forward,
    Backward,
    Align,
    Group,
    Rotate,
    Crop,
    ShapeStyles,
    ShapeFill,
    TextFill,
    TextEffects,
    TextAlign,
}

/// What a control of the tab does.
#[derive(Clone, Debug, PartialEq)]
pub enum FormatAction {
    ChangeShape(TextBoxShape),
    Corrections(f64, f64),
    Recolor(Recolor),
    Saturation(f64),
    Temperature(f64),
    Artistic(Option<&'static str>),
    Transparency(f64),
    TransparentWhite,
    RemoveBackground(bool),
    /// 図の変更: a picture the page's picker brings.
    ChangePicture,
    Reset(bool),
    Outline(OutlineEdit),
    Effect(Effect),
    Place(&'static str, &'static str),
    Wrap(Wrap),
    Arrange(Arrange),
    Align(Align),
    AlignTo(AlignTo),
    Group(bool),
    Rotate(Rotate),
    CropToShape(TextBoxShape),
    Aspect(u32, u32),
    /// サイズ: the height and width, in millimetres.
    Size {
        width_mm: f64,
        height_mm: f64,
    },
    ShapeStyle(usize, usize),
    ShapeFill(Fill),
    TextFill(Paint),
    TextWarp(&'static str),
    TextAlign(&'static str),
    /// トリミング, リンクの作成, 折り返し点の編集, the Selection Pane.
    Ui(ObjectUi),
    /// 図の圧縮 (`word_files`).
    Compress,
}

/// Colours the outline, fill and text lists offer: the Office theme's
/// accents and Word's standard colours, by name.
const COLORS: [(&str, &str); 14] = [
    ("000000", "objects.color.black"),
    ("FFFFFF", "objects.color.white"),
    ("44546A", "objects.color.blue_gray"),
    ("4472C4", "objects.color.blue"),
    ("ED7D31", "objects.color.orange"),
    ("A5A5A5", "objects.color.gray"),
    ("FFC000", "objects.color.gold"),
    ("5B9BD5", "objects.color.light_blue"),
    ("70AD47", "objects.color.green"),
    ("C00000", "objects.color.dark_red"),
    ("FF0000", "objects.color.red"),
    ("FFFF00", "objects.color.yellow"),
    ("00B050", "objects.color.bright_green"),
    ("7030A0", "objects.color.purple"),
];

fn act(action: FormatAction) -> Action {
    Action::ObjectFormat(action)
}

fn popup(which: FormatPopup) -> Popup {
    Popup::ObjectFormat(which)
}

fn japanese() -> bool {
    !i18n::is_english()
}

/// A text button that opens one of the tab's lists.
fn list_button(
    id: &'static str,
    label: &'static str,
    open: bool,
    which: FormatPopup,
    enabled: bool,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .h(px(24.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .when(!enabled, |button| button.text_color(theme::text_faint()))
        .when(enabled, |button| {
            button
                .text_color(theme::text())
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .on_click(listener(Action::Popup(popup(which)), cx))
        })
        .when(open, |button| button.bg(theme::selected()))
        .child(label)
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element()
}

/// A text button that runs one of the tab's actions.
fn text_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    tooltip: &'static str,
    active: bool,
    action: FormatAction,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(24.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(active, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(act(action), cx))
        .child(label.into())
        .into_any_element()
}

fn swatch(
    id: impl Into<gpui::ElementId>,
    color: &str,
    name: &'static str,
    action: FormatAction,
    cx: &mut Context<DocView>,
) -> AnyElement {
    let value = u32::from_str_radix(color, 16).unwrap_or(0);
    div()
        .id(id)
        .size(px(18.0))
        .rounded_sm()
        .bg(gpui::rgb(value))
        .border_1()
        .border_color(theme::border())
        .cursor_pointer()
        .hover(|square| square.border_color(theme::accent()))
        .tooltip(Tooltip::text(t(name)))
        .on_click(listener(act(action), cx))
        .into_any_element()
}

/// A row of colour squares, each running `action` with its colour.
fn color_row(
    id: &'static str,
    action: impl Fn(&str) -> FormatAction,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .w(px(7.0 * 22.0))
        .px_1()
        .py_1()
        .children(
            COLORS
                .iter()
                .enumerate()
                .map(|(index, (color, name))| swatch((id, index), color, name, action(color), cx)),
        )
        .into_any_element()
}

fn percent(value: f64) -> String {
    format!("{:+.0}%", value * 100.0)
}

fn wrap_label(wrap: Wrap) -> &'static str {
    t(match wrap {
        Wrap::Inline => "picture.wrap.inline",
        Wrap::Square => "picture.wrap.square",
        Wrap::Tight => "picture.wrap.tight",
        Wrap::Through => "picture.wrap.through",
        Wrap::TopAndBottom => "picture.wrap.top_bottom",
        Wrap::BehindText => "picture.wrap.behind",
        Wrap::InFrontOfText => "picture.wrap.front",
    })
}

fn artistic_label(effect: &str) -> &'static str {
    match effect {
        "artisticPencilGrayscale" => "objects.artistic.pencil_grayscale",
        "artisticLineDrawing" => "objects.artistic.line_drawing",
        _ => "objects.artistic.blur",
    }
}

fn warp_label(preset: &str) -> &'static str {
    match preset {
        "textNoShape" => "objects.warp.none",
        "textArchUp" => "objects.warp.arch_up",
        "textArchDown" => "objects.warp.arch_down",
        "textCircle" => "objects.warp.circle",
        "textButton" => "objects.warp.button",
        "textWave1" => "objects.warp.wave1",
        "textWave2" => "objects.warp.wave2",
        "textDoubleWave1" => "objects.warp.double_wave",
        "textSlantUp" => "objects.warp.slant_up",
        "textSlantDown" => "objects.warp.slant_down",
        "textTriangle" => "objects.warp.triangle",
        "textTriangleInverted" => "objects.warp.triangle_inverted",
        "textChevron" => "objects.warp.chevron",
        "textChevronInverted" => "objects.warp.chevron_inverted",
        "textInflate" => "objects.warp.inflate",
        "textDeflate" => "objects.warp.deflate",
        "textFadeRight" => "objects.warp.fade_right",
        "textFadeLeft" => "objects.warp.fade_left",
        "textCanUp" => "objects.warp.can_up",
        _ => "objects.warp.can_down",
    }
}

/// The shape gallery, for 図形の変更 (lines left out: a shape does not
/// become one).
fn shape_gallery(cx: &mut Context<DocView>) -> AnyElement {
    let mut panel = menu_panel("change-shape-menu")
        .w(px(300.0))
        .max_h(px(460.0))
        .overflow_y_scroll();
    let mut index = 0usize;
    for category in CATEGORIES.iter() {
        if category.shapes.iter().all(|(shape, _, _)| shape.is_line()) {
            continue;
        }
        panel = panel.child(menu_caption(if japanese() {
            category.ja
        } else {
            category.en
        }));
        for (shape, ja, en) in category.shapes {
            panel = panel.child(menu_row(
                ("change-shape", index),
                if japanese() { *ja } else { *en },
                false,
                act(FormatAction::ChangeShape(*shape)),
                cx,
            ));
            index += 1;
        }
    }
    panel.into_any_element()
}

/// 図の枠線 / 図形の枠線: colour, none, weight, dashes (and arrows for a
/// shape).
fn outline_menu(arrows: bool, cx: &mut Context<DocView>) -> AnyElement {
    let weights = [0.25, 0.5, 0.75, 1.0, 1.5, 2.25, 3.0, 4.5, 6.0];
    let dashes = [
        (Dash::Solid, "objects.dash.solid"),
        (Dash::RoundDot, "objects.dash.round_dot"),
        (Dash::SquareDot, "objects.dash.square_dot"),
        (Dash::Dash, "objects.dash.dash"),
        (Dash::DashDot, "objects.dash.dash_dot"),
        (Dash::LongDash, "objects.dash.long_dash"),
        (Dash::LongDashDot, "objects.dash.long_dash_dot"),
        (Dash::LongDashDotDot, "objects.dash.long_dash_dot_dot"),
    ];
    let arrow = |id: &'static str,
                 key: &'static str,
                 start: Option<&str>,
                 end: Option<&str>,
                 cx: &mut Context<DocView>| {
        menu_row(
            id,
            t(key),
            false,
            act(FormatAction::Outline(OutlineEdit::Arrows(
                start.map(str::to_string),
                end.map(str::to_string),
            ))),
            cx,
        )
    };
    let mut panel = menu_panel("format-outline-menu")
        .w(px(200.0))
        .max_h(px(460.0))
        .overflow_y_scroll()
        .child(color_row(
            "outline-color",
            |color| FormatAction::Outline(OutlineEdit::Color(color.to_string())),
            cx,
        ))
        .child(menu_row(
            "outline-none",
            t("objects.outline.none"),
            false,
            act(FormatAction::Outline(OutlineEdit::None)),
            cx,
        ))
        .child(menu_caption(t("objects.outline.weight")))
        .children(weights.into_iter().enumerate().map(|(index, weight)| {
            menu_row(
                ("outline-weight", index),
                format!("{weight} pt"),
                false,
                act(FormatAction::Outline(OutlineEdit::Width(weight))),
                cx,
            )
        }))
        .child(menu_caption(t("objects.outline.dash")))
        .children(
            dashes
                .into_iter()
                .enumerate()
                .map(|(index, (dash, label))| {
                    menu_row(
                        ("outline-dash", index),
                        t(label),
                        false,
                        act(FormatAction::Outline(OutlineEdit::Dash(dash))),
                        cx,
                    )
                }),
        );
    if arrows {
        panel = panel
            .child(menu_caption(t("objects.outline.arrows")))
            .child(arrow("arrow-none", "objects.arrow.none", None, None, cx))
            .child(arrow(
                "arrow-end",
                "objects.arrow.end",
                None,
                Some("triangle"),
                cx,
            ))
            .child(arrow(
                "arrow-start",
                "objects.arrow.start",
                Some("triangle"),
                None,
                cx,
            ))
            .child(arrow(
                "arrow-both",
                "objects.arrow.both",
                Some("triangle"),
                Some("triangle"),
                cx,
            ));
    }
    panel.into_any_element()
}

/// 図の効果 / 図形の効果: 影, 反射, 光彩, ぼかし, 面取り.
fn effects_menu(cx: &mut Context<DocView>) -> AnyElement {
    let shadows = [
        "objects.shadow.bottom_right",
        "objects.shadow.bottom",
        "objects.shadow.bottom_left",
        "objects.shadow.right",
        "objects.shadow.center",
        "objects.shadow.left",
        "objects.shadow.top_right",
        "objects.shadow.top",
        "objects.shadow.top_left",
    ];
    let inner = [
        "objects.shadow.inner_center",
        "objects.shadow.inner_top_left",
        "objects.shadow.inner_bottom_right",
    ];
    let effect = |id: (&'static str, usize),
                  label: SharedString,
                  effect: Effect,
                  cx: &mut Context<DocView>| {
        menu_row(id, label, false, act(FormatAction::Effect(effect)), cx)
    };
    let mut panel = menu_panel("format-effects-menu")
        .w(px(240.0))
        .max_h(px(480.0))
        .overflow_y_scroll()
        .child(menu_caption(t("objects.effect.shadow")))
        .child(effect(
            ("shadow-none", 0),
            t("objects.effect.none").into(),
            Effect::Shadow(None),
            cx,
        ));
    for (index, (preset, label)) in ShadowPreset::OUTER.iter().zip(shadows).enumerate() {
        panel = panel.child(effect(
            ("shadow", index),
            t(label).into(),
            Effect::Shadow(Some(*preset)),
            cx,
        ));
    }
    for (index, (preset, label)) in ShadowPreset::INNER.iter().zip(inner).enumerate() {
        panel = panel.child(effect(
            ("inner-shadow", index),
            t(label).into(),
            Effect::Shadow(Some(*preset)),
            cx,
        ));
    }
    panel = panel
        .child(menu_caption(t("objects.effect.reflection")))
        .child(effect(
            ("reflection-none", 0),
            t("objects.effect.none").into(),
            Effect::Reflection(None),
            cx,
        ));
    for (index, (preset, label)) in [
        (ReflectionPreset::Tight, "objects.reflection.tight"),
        (ReflectionPreset::Half, "objects.reflection.half"),
        (ReflectionPreset::Full, "objects.reflection.full"),
    ]
    .into_iter()
    .enumerate()
    {
        panel = panel.child(effect(
            ("reflection", index),
            t(label).into(),
            Effect::Reflection(Some(preset)),
            cx,
        ));
    }
    panel = panel
        .child(menu_caption(t("objects.effect.glow")))
        .child(effect(
            ("glow-none", 0),
            t("objects.effect.none").into(),
            Effect::Glow(None),
            cx,
        ));
    for (index, (radius, color)) in [
        (5.0, "4472C4"),
        (8.0, "ED7D31"),
        (11.0, "70AD47"),
        (18.0, "FFC000"),
    ]
    .into_iter()
    .enumerate()
    {
        panel = panel.child(effect(
            ("glow", index),
            i18n::format("objects.glow.size", &[("n", &radius)]).into(),
            Effect::Glow(Some((radius, color.to_string()))),
            cx,
        ));
    }
    panel = panel
        .child(menu_caption(t("objects.effect.soft_edges")))
        .child(effect(
            ("soft-none", 0),
            t("objects.effect.none").into(),
            Effect::SoftEdge(None),
            cx,
        ));
    for (index, radius) in [1.0, 2.5, 5.0, 10.0, 25.0, 50.0].into_iter().enumerate() {
        panel = panel.child(effect(
            ("soft", index),
            format!("{radius} pt").into(),
            Effect::SoftEdge(Some(radius)),
            cx,
        ));
    }
    panel = panel
        .child(menu_caption(t("objects.effect.bevel")))
        .child(effect(
            ("bevel-none", 0),
            t("objects.effect.none").into(),
            Effect::Bevel(None),
            cx,
        ));
    for (index, (preset, label)) in [
        ("circle", "objects.bevel.circle"),
        ("relaxedInset", "objects.bevel.relaxed_inset"),
        ("cross", "objects.bevel.cross"),
        ("coolSlant", "objects.bevel.cool_slant"),
        ("angle", "objects.bevel.angle"),
        ("softRound", "objects.bevel.soft_round"),
    ]
    .into_iter()
    .enumerate()
    {
        panel = panel.child(effect(
            ("bevel", index),
            t(label).into(),
            Effect::Bevel(Some(preset.to_string())),
            cx,
        ));
    }
    panel.into_any_element()
}

/// One square of the style gallery, drawn in its fill and outline.
fn style_swatch(row: usize, column: usize, cx: &mut Context<DocView>) -> AnyElement {
    let accents = [
        0x404040, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5, 0x70AD47,
    ];
    let accent = accents[column];
    let tint = |color: u32, amount: f64| {
        let channel = |shift: u32| {
            let value = ((color >> shift) & 0xff) as f64;
            ((value + (255.0 - value) * amount) as u32).min(255) << shift
        };
        channel(16) | channel(8) | channel(0)
    };
    let (fill, line) = match row {
        0 => (0xFFFFFF, accent),
        1 | 2 => (accent, accent),
        3 => (tint(accent, 0.6), accent),
        4 => (tint(accent, 0.8), tint(accent, 0.8)),
        _ => (tint(accent, 0.0), accent),
    };
    div()
        .id(("format-shape-style", row * 10 + column))
        .size(px(30.0))
        .rounded_sm()
        .bg(gpui::rgb(fill))
        .border_2()
        .border_color(gpui::rgb(line))
        .cursor_pointer()
        .hover(|square| square.border_color(theme::accent()))
        .on_click(listener(act(FormatAction::ShapeStyle(row, column)), cx))
        .into_any_element()
}

impl DocView {
    /// The open list of the ribbon, if it is one of this tab's.
    fn format_popup(&self) -> Option<FormatPopup> {
        match &self.document {
            Document::Pages { ribbon, .. } => match ribbon.popup {
                Some(Popup::ObjectFormat(which)) => Some(which),
                _ => None,
            },
            _ => None,
        }
    }

    /// The selected object's format, for the tab.
    pub(crate) fn selected_object_format(&self) -> Option<ObjectFormat> {
        let Document::Pages {
            document, object, ..
        } = &self.document
        else {
            return None;
        };
        document.object_format(object.as_ref()?.object.target)
    }

    /// The contextual tab for the selected object: whether it is a picture
    /// (図の形式 rather than 図形の書式) and its groups.
    pub(crate) fn object_tab(&self, cx: &mut Context<Self>) -> Option<(bool, Vec<AnyElement>)> {
        let format = self.selected_object_format()?;
        let open = self.format_popup();
        let mut groups = Vec::new();
        let picture = format.sort == ObjectSort::Picture;
        match format.sort {
            ObjectSort::Picture => {
                groups.push(self.adjust_group(open, &format, cx));
                groups.push(picture_style_group(open, cx));
            }
            ObjectSort::Shape | ObjectSort::TextBox | ObjectSort::WordArt => {
                groups.push(self.shape_insert_group(open, &format, cx));
                groups.push(shape_style_group(open, cx));
                groups.push(wordart_group(open, cx));
            }
            ObjectSort::Group | ObjectSort::Chart | ObjectSort::SmartArt => {
                groups.push(picture_style_group(open, cx));
            }
        }
        groups.push(self.arrange_group(open, &format, cx));
        groups.push(self.size_group(open, &format, cx));
        Some((picture, groups))
    }

    /// 図の形式 → 調整.
    fn adjust_group(
        &self,
        open: Option<FormatPopup>,
        format: &ObjectFormat,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_open = |which| open == Some(which);
        let corrections = is_open(FormatPopup::Corrections).then(|| {
            let mut panel = menu_panel("corrections-menu")
                .w(px(330.0))
                .child(menu_caption(t("objects.corrections.caption")));
            for contrast in CORRECTION_STEPS.iter().rev() {
                panel = panel.child(div().flex().gap_1().children(
                    CORRECTION_STEPS.iter().enumerate().map(|(index, bright)| {
                        text_button(
                            SharedString::from(format!("correction-{index}-{contrast}")),
                            format!("{} / {}", percent(*bright), percent(*contrast)),
                            t("objects.corrections.cell"),
                            false,
                            FormatAction::Corrections(*bright, *contrast),
                            cx,
                        )
                    }),
                ));
            }
            panel.into_any_element()
        });
        let color = is_open(FormatPopup::Color).then(|| {
            let recolor = |id: (&'static str, usize),
                           label: SharedString,
                           recolor: Recolor,
                           cx: &mut Context<Self>| {
                menu_row(id, label, false, act(FormatAction::Recolor(recolor)), cx)
            };
            let mut panel = menu_panel("color-menu")
                .w(px(300.0))
                .child(menu_caption(t("objects.color.saturation")))
                .child(row(SATURATIONS
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        text_button(
                            ("saturation", index),
                            format!("{:.0}%", value * 100.0),
                            t("objects.color.saturation"),
                            false,
                            FormatAction::Saturation(*value),
                            cx,
                        )
                    })
                    .collect()))
                .child(menu_caption(t("objects.color.tone")))
                .child(row(TEMPERATURES
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        text_button(
                            ("temperature", index),
                            format!("{value:.0}K"),
                            t("objects.color.tone"),
                            false,
                            FormatAction::Temperature(*value),
                            cx,
                        )
                    })
                    .collect()))
                .child(menu_caption(t("objects.color.recolor")))
                .child(recolor(
                    ("recolor", 0),
                    t("objects.color.none").into(),
                    Recolor::None,
                    cx,
                ))
                .child(recolor(
                    ("recolor", 1),
                    t("objects.color.grayscale").into(),
                    Recolor::Grayscale,
                    cx,
                ))
                .child(recolor(
                    ("recolor", 2),
                    t("objects.color.sepia").into(),
                    Recolor::Sepia,
                    cx,
                ))
                .child(recolor(
                    ("recolor", 3),
                    t("objects.color.black_white").into(),
                    Recolor::BlackAndWhite(0.5),
                    cx,
                ));
            for (index, slot) in ["accent1", "accent2", "accent4", "accent6"]
                .into_iter()
                .enumerate()
            {
                for (light, key) in [
                    (false, "objects.color.accent_dark"),
                    (true, "objects.color.accent_light"),
                ] {
                    panel = panel.child(recolor(
                        ("recolor-accent", index * 2 + usize::from(light)),
                        i18n::format(key, &[("n", &slot[6..].to_string())]).into(),
                        Recolor::Accent {
                            slot: slot.to_string(),
                            light,
                        },
                        cx,
                    ));
                }
            }
            panel
                .child(menu_row(
                    "transparent-white",
                    t("objects.color.transparent_white"),
                    false,
                    act(FormatAction::TransparentWhite),
                    cx,
                ))
                .into_any_element()
        });
        let artistic = is_open(FormatPopup::Artistic).then(|| {
            menu_panel("artistic-menu")
                .w(px(200.0))
                .child(menu_row(
                    "artistic-none",
                    t("objects.artistic.none"),
                    false,
                    act(FormatAction::Artistic(None)),
                    cx,
                ))
                .children(ARTISTIC_EFFECTS.iter().enumerate().map(|(index, effect)| {
                    menu_row(
                        ("artistic", index),
                        t(artistic_label(effect)),
                        false,
                        act(FormatAction::Artistic(Some(effect))),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let transparency = is_open(FormatPopup::Transparency).then(|| {
            menu_panel("transparency-menu")
                .w(px(160.0))
                .children(
                    [0.0, 0.15, 0.3, 0.5, 0.65, 0.8, 0.95]
                        .into_iter()
                        .enumerate()
                        .map(|(index, value)| {
                            menu_row(
                                ("transparency", index),
                                format!("{:.0}%", value * 100.0),
                                false,
                                act(FormatAction::Transparency(value)),
                                cx,
                            )
                        }),
                )
                .into_any_element()
        });
        let change = is_open(FormatPopup::ChangePicture).then(|| {
            menu_panel("change-picture-menu")
                .w(px(200.0))
                .child(menu_row(
                    "change-file",
                    t("objects.change.file"),
                    false,
                    act(FormatAction::ChangePicture),
                    cx,
                ))
                .into_any_element()
        });
        let reset = is_open(FormatPopup::Reset).then(|| {
            menu_panel("reset-menu")
                .w(px(200.0))
                .child(menu_row(
                    "reset",
                    t("objects.reset"),
                    false,
                    act(FormatAction::Reset(false)),
                    cx,
                ))
                .child(menu_row(
                    "reset-size",
                    t("objects.reset_size"),
                    false,
                    act(FormatAction::Reset(true)),
                    cx,
                ))
                .into_any_element()
        });
        let removed = self.word_ref().is_some_and(|document| {
            jade_web::word::objects::object_run(
                jade_web::word::WordSession::word_document(&**document),
                format.target,
            )
            .and_then(|run| run.image.as_ref())
            .is_some_and(|image| {
                image
                    .format()
                    .corrections
                    .as_ref()
                    .is_some_and(|corrections| corrections.background_removal.is_some())
            })
        });
        let list =
            |id: &'static str, key: &'static str, which: FormatPopup, cx: &mut Context<Self>| {
                list_button(id, t(key), is_open(which), which, true, cx)
            };
        group(
            t("objects.group.adjust"),
            vec![
                row(vec![
                    text_button(
                        "remove-background",
                        t(if removed {
                            "objects.background.restore"
                        } else {
                            "objects.background.remove"
                        }),
                        t("objects.background.title"),
                        removed,
                        FormatAction::RemoveBackground(!removed),
                        cx,
                    ),
                    dropdown(
                        list(
                            "corrections",
                            "objects.corrections",
                            FormatPopup::Corrections,
                            cx,
                        ),
                        corrections,
                    ),
                    dropdown(
                        list("color", "objects.color", FormatPopup::Color, cx),
                        color,
                    ),
                    dropdown(
                        list("artistic", "objects.artistic", FormatPopup::Artistic, cx),
                        artistic,
                    ),
                ]),
                row(vec![
                    text_button(
                        "compress-pictures",
                        t("objects.compress"),
                        t("compress.title"),
                        false,
                        FormatAction::Compress,
                        cx,
                    ),
                    dropdown(
                        list(
                            "transparency",
                            "objects.transparency",
                            FormatPopup::Transparency,
                            cx,
                        ),
                        transparency,
                    ),
                    dropdown(
                        list(
                            "change-picture",
                            "objects.change_picture",
                            FormatPopup::ChangePicture,
                            cx,
                        ),
                        change,
                    ),
                    dropdown(
                        list("reset-picture", "objects.reset", FormatPopup::Reset, cx),
                        reset,
                    ),
                ]),
            ],
        )
    }

    /// 図形の書式 → 図形の挿入: 図形の変更, リンクの作成.
    fn shape_insert_group(
        &self,
        open: Option<FormatPopup>,
        format: &ObjectFormat,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let gallery = (open == Some(FormatPopup::ChangeShape)).then(|| shape_gallery(cx));
        let modes = self.object_modes();
        let mut second = Vec::new();
        if format.sort != ObjectSort::Shape {
            second.push(text_button(
                "format-link-text-box",
                t("objects.link"),
                t("objects.link_title"),
                modes.linking,
                FormatAction::Ui(ObjectUi::LinkTextBox),
                cx,
            ));
        }
        group(
            t("objects.group.insert_shapes"),
            vec![
                row(vec![dropdown(
                    list_button(
                        "change-shape",
                        t("objects.change_shape"),
                        open == Some(FormatPopup::ChangeShape),
                        FormatPopup::ChangeShape,
                        true,
                        cx,
                    ),
                    gallery,
                )]),
                row(second),
            ],
        )
    }

    /// 配置: 位置, 文字列の折り返し, 前面へ, 背面へ, オブジェクトの選択と
    /// 表示, 配置, グループ化, 回転.
    fn arrange_group(
        &self,
        open: Option<FormatPopup>,
        format: &ObjectFormat,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_open = |which| open == Some(which);
        let floating = format.wrap != Wrap::Inline;
        let modes = self.object_modes();
        let position = is_open(FormatPopup::Position).then(|| {
            let spots = [
                ("left", "top", "objects.position.top_left"),
                ("center", "top", "objects.position.top_center"),
                ("right", "top", "objects.position.top_right"),
                ("left", "center", "objects.position.middle_left"),
                ("center", "center", "objects.position.middle_center"),
                ("right", "center", "objects.position.middle_right"),
                ("left", "bottom", "objects.position.bottom_left"),
                ("center", "bottom", "objects.position.bottom_center"),
                ("right", "bottom", "objects.position.bottom_right"),
            ];
            menu_panel("position-menu")
                .w(px(250.0))
                .child(menu_row(
                    "position-inline",
                    t("picture.wrap.inline"),
                    !floating,
                    act(FormatAction::Wrap(Wrap::Inline)),
                    cx,
                ))
                .child(menu_caption(t("objects.position.with_wrapping")))
                .children(spots.into_iter().enumerate().map(|(index, (h, v, label))| {
                    menu_row(
                        ("position", index),
                        t(label),
                        false,
                        act(FormatAction::Place(h, v)),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let wrap = is_open(FormatPopup::Wrap).then(|| {
            let mut panel =
                menu_panel("object-wrap-menu")
                    .w(px(220.0))
                    .children(WRAPS.iter().enumerate().map(|(index, wrap)| {
                        menu_row(
                            ("object-wrap", index),
                            wrap_label(*wrap),
                            format.wrap == *wrap,
                            act(FormatAction::Wrap(*wrap)),
                            cx,
                        )
                    }));
            if matches!(format.wrap, Wrap::Tight | Wrap::Through) {
                panel = panel.child(menu_row(
                    "format-wrap-points",
                    t("objects.wrap_points"),
                    modes.wrap_points,
                    act(FormatAction::Ui(ObjectUi::EditWrapPoints)),
                    cx,
                ));
            }
            panel.into_any_element()
        });
        let order =
            |id: &'static str, entries: [(Arrange, &'static str); 3], cx: &mut Context<Self>| {
                menu_panel(id)
                    .w(px(200.0))
                    .children(
                        entries
                            .into_iter()
                            .enumerate()
                            .map(|(index, (arrange, key))| {
                                menu_row(
                                    (id, index),
                                    t(key),
                                    false,
                                    act(FormatAction::Arrange(arrange)),
                                    cx,
                                )
                            }),
                    )
                    .into_any_element()
            };
        let forward = is_open(FormatPopup::Forward).then(|| {
            order(
                "forward-menu",
                [
                    (Arrange::Forward, "objects.forward"),
                    (Arrange::ToFront, "objects.to_front"),
                    (Arrange::InFrontOfText, "objects.in_front_of_text"),
                ],
                cx,
            )
        });
        let backward = is_open(FormatPopup::Backward).then(|| {
            order(
                "backward-menu",
                [
                    (Arrange::Backward, "objects.backward"),
                    (Arrange::ToBack, "objects.to_back"),
                    (Arrange::BehindText, "objects.behind_text"),
                ],
                cx,
            )
        });
        let align_to = self.objects_ui.align_to.unwrap_or(AlignTo::Page);
        let align = is_open(FormatPopup::Align).then(|| {
            let aligns = [
                (Align::Left, "objects.align.left"),
                (Align::Center, "objects.align.center"),
                (Align::Right, "objects.align.right"),
                (Align::Top, "objects.align.top"),
                (Align::Middle, "objects.align.middle"),
                (Align::Bottom, "objects.align.bottom"),
                (Align::DistributeHorizontally, "objects.align.distribute_h"),
                (Align::DistributeVertically, "objects.align.distribute_v"),
            ];
            menu_panel("align-menu")
                .w(px(230.0))
                .children(
                    aligns
                        .into_iter()
                        .enumerate()
                        .map(|(index, (align, label))| {
                            menu_row(
                                ("align", index),
                                t(label),
                                false,
                                act(FormatAction::Align(align)),
                                cx,
                            )
                        }),
                )
                .child(menu_caption(t("objects.align.relative")))
                .children(
                    [
                        (AlignTo::Page, "objects.align.page"),
                        (AlignTo::Margin, "objects.align.margin"),
                        (AlignTo::Selection, "objects.align.selection"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (to, label))| {
                        menu_row(
                            ("align-to", index),
                            t(label),
                            align_to == to,
                            act(FormatAction::AlignTo(to)),
                            cx,
                        )
                    }),
                )
                .into_any_element()
        });
        let group_menu = is_open(FormatPopup::Group).then(|| {
            menu_panel("group-menu")
                .w(px(180.0))
                .child(menu_row(
                    "group",
                    t("objects.group"),
                    false,
                    act(FormatAction::Group(true)),
                    cx,
                ))
                .child(menu_row(
                    "ungroup",
                    t("objects.ungroup"),
                    false,
                    act(FormatAction::Group(false)),
                    cx,
                ))
                .into_any_element()
        });
        let rotate = is_open(FormatPopup::Rotate).then(|| {
            menu_panel("rotate-menu")
                .w(px(220.0))
                .children(
                    [
                        (Rotate::Right90, "objects.rotate.right", false),
                        (Rotate::Left90, "objects.rotate.left", false),
                        (
                            Rotate::FlipVertical,
                            "objects.rotate.flip_v",
                            format.flip_vertical,
                        ),
                        (
                            Rotate::FlipHorizontal,
                            "objects.rotate.flip_h",
                            format.flip_horizontal,
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (rotate, label, on))| {
                        menu_row(
                            ("rotate", index),
                            t(label),
                            on,
                            act(FormatAction::Rotate(rotate)),
                            cx,
                        )
                    }),
                )
                .into_any_element()
        });
        let list = |id: &'static str,
                    key: &'static str,
                    which: FormatPopup,
                    enabled: bool,
                    cx: &mut Context<Self>| {
            list_button(id, t(key), is_open(which), which, enabled, cx)
        };
        group(
            t("objects.group.arrange"),
            vec![
                row(vec![
                    dropdown(
                        list(
                            "object-position",
                            "objects.position",
                            FormatPopup::Position,
                            true,
                            cx,
                        ),
                        position,
                    ),
                    dropdown(
                        list("object-wrap", "picture.wrap", FormatPopup::Wrap, true, cx),
                        wrap,
                    ),
                    dropdown(
                        list(
                            "bring-forward",
                            "objects.forward",
                            FormatPopup::Forward,
                            floating,
                            cx,
                        ),
                        forward,
                    ),
                    dropdown(
                        list(
                            "send-backward",
                            "objects.backward",
                            FormatPopup::Backward,
                            floating,
                            cx,
                        ),
                        backward,
                    ),
                ]),
                row(vec![
                    text_button(
                        "format-selection-pane",
                        t("objects.selection_pane"),
                        t("objects.selection_pane_title"),
                        modes.selection_pane,
                        FormatAction::Ui(ObjectUi::SelectionPane),
                        cx,
                    ),
                    dropdown(
                        list(
                            "align-objects",
                            "objects.align",
                            FormatPopup::Align,
                            floating,
                            cx,
                        ),
                        align,
                    ),
                    dropdown(
                        list(
                            "group-objects",
                            "objects.group",
                            FormatPopup::Group,
                            true,
                            cx,
                        ),
                        group_menu,
                    ),
                    dropdown(
                        list(
                            "rotate-objects",
                            "objects.rotate",
                            FormatPopup::Rotate,
                            true,
                            cx,
                        ),
                        rotate,
                    ),
                ]),
            ],
        )
    }

    /// サイズ: トリミング and the height and width.
    fn size_group(
        &self,
        open: Option<FormatPopup>,
        format: &ObjectFormat,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let picture = format.sort == ObjectSort::Picture;
        let modes = self.object_modes();
        let crop = (open == Some(FormatPopup::Crop)).then(|| {
            menu_panel("crop-menu")
                .id("crop-menu")
                .w(px(260.0))
                .max_h(px(440.0))
                .overflow_y_scroll()
                .child(menu_row(
                    "crop",
                    t("objects.crop"),
                    modes.cropping,
                    act(FormatAction::Ui(ObjectUi::ToggleCrop)),
                    cx,
                ))
                .child(menu_caption(t("objects.crop.aspect")))
                .children(ASPECT_RATIOS.iter().enumerate().map(|(index, (w, h))| {
                    menu_row(
                        ("aspect", index),
                        format!("{w}:{h}"),
                        false,
                        act(FormatAction::Aspect(*w, *h)),
                        cx,
                    )
                }))
                .child(menu_caption(t("objects.crop.shape")))
                .children(crop_shapes().enumerate().map(|(index, (shape, ja, en))| {
                    menu_row(
                        ("crop-shape", index),
                        if japanese() { ja } else { en },
                        false,
                        act(FormatAction::CropToShape(shape)),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let (width_mm, height_mm) = (format.width / PX_PER_MM, format.height / PX_PER_MM);
        let stepper = |id: &'static str,
                       label: &'static str,
                       value: f64,
                       width: bool,
                       cx: &mut Context<Self>| {
            let step = |delta: f64| {
                if width {
                    FormatAction::Size {
                        width_mm: (value + delta).max(1.0),
                        height_mm,
                    }
                } else {
                    FormatAction::Size {
                        width_mm,
                        height_mm: (value + delta).max(1.0),
                    }
                }
            };
            div()
                .flex()
                .items_center()
                .gap_1()
                .text_xs()
                .child(div().w(px(28.0)).text_color(theme::text_dim()).child(label))
                .child(text_button(
                    (id, 0usize),
                    "−",
                    t("objects.size.smaller"),
                    false,
                    step(-1.0),
                    cx,
                ))
                .child(div().w(px(52.0)).child(format!("{value:.1} mm")))
                .child(text_button(
                    (id, 1usize),
                    "+",
                    t("objects.size.larger"),
                    false,
                    step(1.0),
                    cx,
                ))
                .into_any_element()
        };
        group(
            t("objects.group.size"),
            vec![
                row(vec![
                    dropdown(
                        list_button(
                            "crop-picture",
                            t("objects.crop"),
                            open == Some(FormatPopup::Crop),
                            FormatPopup::Crop,
                            picture,
                            cx,
                        ),
                        crop,
                    ),
                    stepper("height", t("objects.size.height"), height_mm, false, cx),
                ]),
                row(vec![stepper(
                    "width",
                    t("objects.size.width"),
                    width_mm,
                    true,
                    cx,
                )]),
            ],
        )
    }

    /// Runs a control of the tab on the selected object.
    pub(crate) fn object_format_action(&mut self, action: FormatAction, cx: &mut Context<Self>) {
        if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.popup = None;
        }
        let selected = match &self.document {
            Document::Pages { object, .. } => object.map(|grab| grab.object),
            _ => None,
        };
        let Some(selected) = selected else {
            return self.word_result(Err(t("objects.nothing_selected").to_string()), cx);
        };
        let target = selected.target;
        match action {
            FormatAction::Ui(ui) => return self.object_ui_action(ui, cx),
            FormatAction::AlignTo(to) => {
                self.objects_ui.align_to = Some(to);
                return cx.notify();
            }
            FormatAction::ChangePicture => {
                self.tools.picking = Some(Pick::ChangePicture);
                return self.host.pick();
            }
            FormatAction::Compress => return self.open_compress_dialog(cx),
            FormatAction::Wrap(wrap) => {
                use jade_web::word::picture::WordPicture;
                self.word_edit(cx, move |document| {
                    document.set_picture_wrap(selected, wrap)
                });
                return self.refresh_selected_object();
            }
            _ => {}
        }
        let to = self.objects_ui.align_to.unwrap_or(AlignTo::Page);
        self.word_edit(cx, move |document| match action {
            FormatAction::ChangeShape(shape) => document.change_shape(target, shape),
            FormatAction::Corrections(brightness, contrast) => {
                document.set_picture_corrections(target, brightness, contrast)
            }
            FormatAction::Recolor(recolor) => document.recolor_picture(target, recolor),
            FormatAction::Saturation(value) => document.set_picture_saturation(target, value),
            FormatAction::Temperature(value) => document.set_picture_temperature(target, value),
            FormatAction::Artistic(effect) => document.set_artistic_effect(target, effect),
            FormatAction::Transparency(value) => document.set_picture_transparency(target, value),
            FormatAction::TransparentWhite => document.set_transparent_color(target, "FFFFFF"),
            FormatAction::RemoveBackground(remove) => {
                document.remove_picture_background(target, remove)
            }
            FormatAction::Reset(size) => document.reset_picture(target, size),
            FormatAction::Outline(edit) => document.set_picture_outline(target, edit),
            FormatAction::Effect(effect) => document.set_object_effect(target, effect),
            FormatAction::Place(h, v) => document.place_object_on_page(target, h, v),
            FormatAction::Arrange(arrange) => document.arrange_object(target, arrange),
            FormatAction::Align(align) => document.align_objects(&[selected], align, to),
            FormatAction::Group(true) => {
                use jade_web::word::group::WordGroups;
                document.group_objects(&[selected])
            }
            FormatAction::Group(false) => {
                use jade_web::word::group::WordGroups;
                document.ungroup_object(selected)
            }
            FormatAction::Rotate(rotate) => document.rotate_object(target, rotate),
            FormatAction::CropToShape(shape) => document.crop_picture_to_shape(target, shape),
            FormatAction::Aspect(width, height) => {
                document.crop_picture_to_aspect(target, (width, height))
            }
            FormatAction::Size {
                width_mm,
                height_mm,
            } => document.resize_object(target, width_mm * PX_PER_MM, height_mm * PX_PER_MM),
            FormatAction::ShapeStyle(row, column) => {
                document.apply_shape_style(target, ShapeStyle::gallery(row, column))
            }
            FormatAction::ShapeFill(fill) => document.set_shape_fill(target, fill),
            FormatAction::TextFill(paint) => document.set_shape_text_color(target, paint),
            FormatAction::TextWarp(preset) => document.set_text_warp(target, preset),
            FormatAction::TextAlign(align) => document.set_text_anchor(target, align),
            FormatAction::Ui(_)
            | FormatAction::AlignTo(_)
            | FormatAction::ChangePicture
            | FormatAction::Compress
            | FormatAction::Wrap(_) => Ok(()),
        });
        self.refresh_selected_object();
    }

    /// 図の変更: the picture the page's picker brought, in place of the
    /// selected one.
    pub(crate) fn change_selected_picture(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let target = match &self.document {
            Document::Pages { object, .. } => object.map(|grab| grab.object.target),
            _ => None,
        };
        let Some(target) = target else {
            return;
        };
        self.word_edit(cx, move |document| document.replace_picture(target, bytes));
        self.refresh_selected_object();
    }
}

/// 図のスタイル: 図の枠線 and 図の効果.
fn picture_style_group(open: Option<FormatPopup>, cx: &mut Context<DocView>) -> AnyElement {
    let is_open = |which| open == Some(which);
    let outline = is_open(FormatPopup::Outline).then(|| outline_menu(false, cx));
    let effects = is_open(FormatPopup::Effects).then(|| effects_menu(cx));
    group(
        t("objects.group.picture_styles"),
        vec![
            row(vec![dropdown(
                list_button(
                    "picture-outline",
                    t("objects.outline"),
                    is_open(FormatPopup::Outline),
                    FormatPopup::Outline,
                    true,
                    cx,
                ),
                outline,
            )]),
            row(vec![dropdown(
                list_button(
                    "picture-effects",
                    t("objects.effects"),
                    is_open(FormatPopup::Effects),
                    FormatPopup::Effects,
                    true,
                    cx,
                ),
                effects,
            )]),
        ],
    )
}

/// 図形のスタイル: the theme style gallery, 塗りつぶし, 枠線, 効果.
fn shape_style_group(open: Option<FormatPopup>, cx: &mut Context<DocView>) -> AnyElement {
    let is_open = |which| open == Some(which);
    let styles = is_open(FormatPopup::ShapeStyles).then(|| {
        let mut panel = menu_panel("shape-styles-menu").w(px(7.0 * 34.0 + 12.0));
        for style_row in 0..ShapeStyle::GALLERY_ROWS {
            panel = panel.child(div().flex().gap_1().py_0p5().children(
                (0..STYLE_COLORS.len()).map(|column| style_swatch(style_row, column, cx)),
            ));
        }
        panel.into_any_element()
    });
    let fill = is_open(FormatPopup::ShapeFill).then(|| {
        menu_panel("shape-fill-menu")
            .w(px(180.0))
            .child(color_row(
                "fill-color",
                |color| FormatAction::ShapeFill(Fill::Paint(Paint::Rgb(color.to_string()))),
                cx,
            ))
            .child(menu_row(
                "fill-none",
                t("objects.fill.none"),
                false,
                act(FormatAction::ShapeFill(Fill::None)),
                cx,
            ))
            .child(menu_caption(t("objects.transparency")))
            .children(
                [0.0, 0.25, 0.5, 0.75]
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| {
                        menu_row(
                            ("fill-transparency", index),
                            format!("{:.0}%", value * 100.0),
                            false,
                            act(FormatAction::ShapeFill(Fill::Transparency(value))),
                            cx,
                        )
                    }),
            )
            .into_any_element()
    });
    let outline = is_open(FormatPopup::Outline).then(|| outline_menu(true, cx));
    let effects = is_open(FormatPopup::Effects).then(|| effects_menu(cx));
    group(
        t("objects.group.shape_styles"),
        vec![
            row(vec![
                dropdown(
                    list_button(
                        "shape-styles",
                        t("objects.shape_styles"),
                        is_open(FormatPopup::ShapeStyles),
                        FormatPopup::ShapeStyles,
                        true,
                        cx,
                    ),
                    styles,
                ),
                dropdown(
                    list_button(
                        "shape-fill",
                        t("objects.fill"),
                        is_open(FormatPopup::ShapeFill),
                        FormatPopup::ShapeFill,
                        true,
                        cx,
                    ),
                    fill,
                ),
            ]),
            row(vec![
                dropdown(
                    list_button(
                        "shape-outline",
                        t("objects.shape_outline"),
                        is_open(FormatPopup::Outline),
                        FormatPopup::Outline,
                        true,
                        cx,
                    ),
                    outline,
                ),
                dropdown(
                    list_button(
                        "shape-effects",
                        t("objects.shape_effects"),
                        is_open(FormatPopup::Effects),
                        FormatPopup::Effects,
                        true,
                        cx,
                    ),
                    effects,
                ),
            ]),
        ],
    )
}

/// ワードアートのスタイル and テキスト: text fill, 変形, 文字の配置.
fn wordart_group(open: Option<FormatPopup>, cx: &mut Context<DocView>) -> AnyElement {
    let is_open = |which| open == Some(which);
    let text_fill = is_open(FormatPopup::TextFill).then(|| {
        menu_panel("text-fill-menu")
            .w(px(180.0))
            .child(color_row(
                "text-color",
                |color| FormatAction::TextFill(Paint::Rgb(color.to_string())),
                cx,
            ))
            .into_any_element()
    });
    let effects = is_open(FormatPopup::TextEffects).then(|| {
        menu_panel("text-warp-menu")
            .id("text-warp-menu")
            .w(px(220.0))
            .max_h(px(420.0))
            .overflow_y_scroll()
            .child(menu_caption(t("objects.warp")))
            .children(TEXT_WARPS.iter().enumerate().map(|(index, preset)| {
                menu_row(
                    ("warp", index),
                    t(warp_label(preset)),
                    false,
                    act(FormatAction::TextWarp(preset)),
                    cx,
                )
            }))
            .into_any_element()
    });
    let align = is_open(FormatPopup::TextAlign).then(|| {
        menu_panel("text-align-menu")
            .w(px(140.0))
            .children(
                [
                    ("top", "objects.text.top"),
                    ("center", "objects.text.middle"),
                    ("bottom", "objects.text.bottom"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (align, key))| {
                    menu_row(
                        ("text-align", index),
                        t(key),
                        false,
                        act(FormatAction::TextAlign(align)),
                        cx,
                    )
                }),
            )
            .into_any_element()
    });
    group(
        t("objects.group.wordart_styles"),
        vec![
            row(vec![
                dropdown(
                    list_button(
                        "text-fill",
                        t("objects.text_fill"),
                        is_open(FormatPopup::TextFill),
                        FormatPopup::TextFill,
                        true,
                        cx,
                    ),
                    text_fill,
                ),
                dropdown(
                    list_button(
                        "text-effects",
                        t("objects.text_effects"),
                        is_open(FormatPopup::TextEffects),
                        FormatPopup::TextEffects,
                        true,
                        cx,
                    ),
                    effects,
                ),
            ]),
            row(vec![dropdown(
                list_button(
                    "text-align",
                    t("objects.text_align"),
                    is_open(FormatPopup::TextAlign),
                    FormatPopup::TextAlign,
                    true,
                    cx,
                ),
                align,
            )]),
        ],
    )
}

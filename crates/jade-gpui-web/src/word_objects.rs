//! The browser shell's drawing-object tools, over the same commands as the
//! native app (`jade-office-model`'s `word::objects`, `picture_format`,
//! `shapes`, `diagrams`): 挿入's 図形, テキスト ボックス, ワードアート,
//! SmartArt and グラフ, and for the picture or shape selected its position,
//! order, rotation, effects, corrections, crop to shape, reset and styles.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_web::editing::EditableDocument;
use jade_web::word::diagrams::{ChartKind, WordDiagrams, SMARTART_LAYOUTS};
use jade_web::word::objects::{Arrange, Rotate, WordObjects};
use jade_web::word::picture::PlacedObject;
use jade_web::word::picture_format::{
    Effect, Recolor, ReflectionPreset, ShadowPreset, WordPictureFormat,
};
use jade_web::word::shape_gallery::CATEGORIES;
use jade_web::word::shapes::{ShapeStyle, TextBoxShape, WordArtStyle, WordShapes};

use crate::i18n::{self, text as t};
use crate::palette;
use crate::ribbon::{
    dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup,
};
use crate::tooltip::Tooltip;
use crate::view::DocView;

/// The object tools' lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectPopup {
    Shapes,
    Insert,
    Arrange,
    Effects,
    Adjust,
}

/// What an object control does.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectAction {
    Shape(TextBoxShape),
    TextBox,
    WordArt(WordArtStyle),
    SmartArt(&'static str),
    Chart(ChartKind),
    Place(&'static str, &'static str),
    Arrange(Arrange),
    Rotate(Rotate),
    Effect(Effect),
    Corrections(f64, f64),
    Recolor(Recolor),
    CropToShape(TextBoxShape),
    Reset,
    ShapeStyle(usize, usize),
}

fn act(action: ObjectAction) -> Action {
    Action::Object(action)
}

fn english() -> bool {
    i18n::is_english()
}

fn list_button(
    id: &'static str,
    label: &'static str,
    open: bool,
    popup: ObjectPopup,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(palette::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(open, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .tooltip(Tooltip::text(label))
        .on_click(listener(Action::Popup(Popup::Object(popup)), cx))
        .child(label)
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(palette::dim()),
        )
        .into_any_element()
}

fn item(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    action: ObjectAction,
    cx: &mut Context<DocView>,
) -> AnyElement {
    menu_row(id, label, false, act(action), cx)
}

/// The 図形 list: Word's gallery, category by category.
fn shapes_list(cx: &mut Context<DocView>) -> AnyElement {
    let mut panel = menu_panel("web-object-shapes")
        .w(px(260.0))
        .max_h(px(420.0))
        .overflow_y_scroll();
    let mut index = 0usize;
    for category in CATEGORIES.iter() {
        panel = panel.child(menu_caption(if english() {
            category.en
        } else {
            category.ja
        }));
        for (shape, ja, en) in category.shapes.iter() {
            let label = if english() { *en } else { *ja };
            panel = panel.child(item(
                ("web-object-shape", index),
                label,
                ObjectAction::Shape(*shape),
                cx,
            ));
            index += 1;
        }
    }
    panel.into_any_element()
}

/// テキスト ボックス, ワードアート, SmartArt and グラフ.
fn insert_list(cx: &mut Context<DocView>) -> AnyElement {
    let mut panel = menu_panel("web-object-insert")
        .w(px(260.0))
        .child(item(
            "web-text-box",
            t("objects.text_box"),
            ObjectAction::TextBox,
            cx,
        ))
        .child(menu_caption(t("objects.wordart")));
    for (index, (style, key)) in [
        (WordArtStyle::BlackShadow, "objects.wordart.black_shadow"),
        (WordArtStyle::AccentShadow, "objects.wordart.accent_shadow"),
        (
            WordArtStyle::WhiteOutlineGlow,
            "objects.wordart.white_outline_glow",
        ),
        (WordArtStyle::OutlineOnly, "objects.wordart.outline_only"),
    ]
    .into_iter()
    .enumerate()
    {
        panel = panel.child(item(
            ("web-wordart", index),
            t(key),
            ObjectAction::WordArt(style),
            cx,
        ));
    }
    panel = panel.child(menu_caption(t("objects.smartart")));
    for (index, (layout, ja, en, _)) in SMARTART_LAYOUTS.iter().enumerate() {
        let label = if english() { *en } else { *ja };
        panel = panel.child(item(
            ("web-smartart", index),
            label,
            ObjectAction::SmartArt(layout),
            cx,
        ));
    }
    panel = panel.child(menu_caption(t("objects.chart")));
    for (index, (kind, key)) in [
        (ChartKind::Bar, "objects.chart.column"),
        (ChartKind::Line, "objects.chart.line"),
        (ChartKind::Pie, "objects.chart.pie"),
    ]
    .into_iter()
    .enumerate()
    {
        panel = panel.child(item(
            ("web-chart", index),
            t(key),
            ObjectAction::Chart(kind),
            cx,
        ));
    }
    panel.into_any_element()
}

/// 位置, 前面へ / 背面へ and 回転.
fn arrange_list(cx: &mut Context<DocView>) -> AnyElement {
    let mut panel = menu_panel("web-object-arrange")
        .w(px(320.0))
        .max_h(px(420.0))
        .overflow_y_scroll()
        .child(menu_caption(t("objects.position")));
    for (index, (h, v, key)) in [
        ("left", "top", "objects.position.top_left"),
        ("center", "top", "objects.position.top_center"),
        ("right", "top", "objects.position.top_right"),
        ("left", "center", "objects.position.middle_left"),
        ("center", "center", "objects.position.middle_center"),
        ("right", "center", "objects.position.middle_right"),
        ("left", "bottom", "objects.position.bottom_left"),
        ("center", "bottom", "objects.position.bottom_center"),
        ("right", "bottom", "objects.position.bottom_right"),
    ]
    .into_iter()
    .enumerate()
    {
        panel = panel.child(item(
            ("web-place", index),
            t(key),
            ObjectAction::Place(h, v),
            cx,
        ));
    }
    panel = panel.child(menu_caption(t("objects.group.arrange")));
    for (index, (arrange, key)) in [
        (Arrange::ToFront, "objects.to_front"),
        (Arrange::Forward, "objects.forward"),
        (Arrange::Backward, "objects.backward"),
        (Arrange::ToBack, "objects.to_back"),
        (Arrange::InFrontOfText, "objects.in_front_of_text"),
        (Arrange::BehindText, "objects.behind_text"),
    ]
    .into_iter()
    .enumerate()
    {
        panel = panel.child(item(
            ("web-order", index),
            t(key),
            ObjectAction::Arrange(arrange),
            cx,
        ));
    }
    panel = panel.child(menu_caption(t("objects.rotate")));
    for (index, (rotate, key)) in [
        (Rotate::Right90, "objects.rotate.right"),
        (Rotate::Left90, "objects.rotate.left"),
        (Rotate::FlipVertical, "objects.rotate.flip_v"),
        (Rotate::FlipHorizontal, "objects.rotate.flip_h"),
    ]
    .into_iter()
    .enumerate()
    {
        panel = panel.child(item(
            ("web-rotate", index),
            t(key),
            ObjectAction::Rotate(rotate),
            cx,
        ));
    }
    panel.into_any_element()
}

/// 影, 反射, 光彩, ぼかし and 面取り.
fn effects_list(cx: &mut Context<DocView>) -> AnyElement {
    let entries: Vec<(&'static str, Effect)> = vec![
        (
            "objects.shadow.bottom_right",
            Effect::Shadow(Some(ShadowPreset::OUTER[0])),
        ),
        (
            "objects.shadow.bottom",
            Effect::Shadow(Some(ShadowPreset::OUTER[1])),
        ),
        ("objects.effect.no_shadow", Effect::Shadow(None)),
        (
            "objects.reflection.tight",
            Effect::Reflection(Some(ReflectionPreset::Tight)),
        ),
        (
            "objects.reflection.half",
            Effect::Reflection(Some(ReflectionPreset::Half)),
        ),
        (
            "objects.reflection.full",
            Effect::Reflection(Some(ReflectionPreset::Full)),
        ),
        ("objects.effect.no_reflection", Effect::Reflection(None)),
        (
            "objects.effect.glow",
            Effect::Glow(Some((8.0, "4472C4".into()))),
        ),
        ("objects.effect.no_glow", Effect::Glow(None)),
        ("objects.effect.soft_edges", Effect::SoftEdge(Some(5.0))),
        ("objects.effect.no_soft_edges", Effect::SoftEdge(None)),
        ("objects.bevel.circle", Effect::Bevel(Some("circle".into()))),
        ("objects.effect.no_bevel", Effect::Bevel(None)),
    ];
    let mut panel = menu_panel("web-object-effects").w(px(220.0));
    for (index, (key, effect)) in entries.into_iter().enumerate() {
        panel = panel.child(item(
            ("web-effect", index),
            t(key),
            ObjectAction::Effect(effect),
            cx,
        ));
    }
    panel.into_any_element()
}

/// A picture's 修整, 色, 図形に合わせてトリミング and リセット; a shape's
/// 図形のスタイル.
fn adjust_list(picture: bool, cx: &mut Context<DocView>) -> AnyElement {
    let mut panel = menu_panel("web-object-adjust")
        .w(px(240.0))
        .max_h(px(420.0))
        .overflow_y_scroll();
    if picture {
        panel = panel.child(menu_caption(t("objects.corrections.caption")));
        for (index, (brightness, contrast)) in
            [(-0.2, 0.0), (0.2, 0.0), (0.0, -0.2), (0.0, 0.2), (0.0, 0.0)]
                .into_iter()
                .enumerate()
        {
            let label = format!("{:+.0}% / {:+.0}%", brightness * 100.0, contrast * 100.0);
            panel = panel.child(item(
                ("web-correction", index),
                label,
                ObjectAction::Corrections(brightness, contrast),
                cx,
            ));
        }
        panel = panel.child(menu_caption(t("objects.color.recolor")));
        for (index, (recolor, key)) in [
            (Recolor::None, "objects.color.none"),
            (Recolor::Grayscale, "objects.color.grayscale"),
            (Recolor::Sepia, "objects.color.sepia"),
        ]
        .into_iter()
        .enumerate()
        {
            panel = panel.child(item(
                ("web-recolor", index),
                t(key),
                ObjectAction::Recolor(recolor),
                cx,
            ));
        }
        panel = panel.child(menu_caption(t("objects.crop.shape")));
        for (index, (shape, ja, en)) in jade_web::word::shape_gallery::crop_shapes()
            .take(12)
            .enumerate()
        {
            let label = if english() { en } else { ja };
            panel = panel.child(item(
                ("web-crop", index),
                label,
                ObjectAction::CropToShape(shape),
                cx,
            ));
        }
        panel = panel.child(item(
            "web-reset",
            t("objects.reset"),
            ObjectAction::Reset,
            cx,
        ));
    } else {
        panel = panel.child(menu_caption(t("objects.shape_styles")));
        for row_index in 0..6 {
            for column in 0..7 {
                let label = format!(
                    "{} {}-{}",
                    t("objects.shape_styles"),
                    row_index + 1,
                    column + 1
                );
                panel = panel.child(item(
                    ("web-style", row_index * 7 + column),
                    label,
                    ObjectAction::ShapeStyle(row_index, column),
                    cx,
                ));
            }
        }
    }
    panel.into_any_element()
}

/// 挿入's drawing group, and with an object selected its tools.
pub fn objects(
    open: Option<Popup>,
    selected: Option<PlacedObject>,
    cx: &mut Context<DocView>,
) -> AnyElement {
    let is_open = |which| open == Some(Popup::Object(which));
    let inserts = vec![
        dropdown(
            list_button(
                "web-object-shapes-button",
                t("objects.shapes"),
                is_open(ObjectPopup::Shapes),
                ObjectPopup::Shapes,
                cx,
            ),
            is_open(ObjectPopup::Shapes).then(|| shapes_list(cx)),
        ),
        dropdown(
            list_button(
                "web-object-insert-button",
                t("objects.smartart"),
                is_open(ObjectPopup::Insert),
                ObjectPopup::Insert,
                cx,
            ),
            is_open(ObjectPopup::Insert).then(|| insert_list(cx)),
        ),
    ];
    let mut lines = vec![row(inserts)];
    if let Some(selected) = selected {
        let adjust = if selected.is_picture() {
            t("objects.group.adjust")
        } else {
            t("objects.shape_styles")
        };
        lines.push(row(vec![
            dropdown(
                list_button(
                    "web-object-arrange-button",
                    t("objects.group.arrange"),
                    is_open(ObjectPopup::Arrange),
                    ObjectPopup::Arrange,
                    cx,
                ),
                is_open(ObjectPopup::Arrange).then(|| arrange_list(cx)),
            ),
            dropdown(
                list_button(
                    "web-object-effects-button",
                    t("objects.effects"),
                    is_open(ObjectPopup::Effects),
                    ObjectPopup::Effects,
                    cx,
                ),
                is_open(ObjectPopup::Effects).then(|| effects_list(cx)),
            ),
            dropdown(
                list_button(
                    "web-object-adjust-button",
                    adjust,
                    is_open(ObjectPopup::Adjust),
                    ObjectPopup::Adjust,
                    cx,
                ),
                is_open(ObjectPopup::Adjust).then(|| adjust_list(selected.is_picture(), cx)),
            ),
        ]));
    }
    group(t("objects.shapes"), lines)
}

/// Runs an object control on the document, for the object selected.
pub fn run(
    document: &mut EditableDocument,
    selected: Option<PlacedObject>,
    action: ObjectAction,
) -> Result<(), String> {
    let need = || {
        selected
            .map(|object| object.target)
            .ok_or_else(|| t("objects.nothing_selected").to_string())
    };
    match action {
        ObjectAction::Shape(shape) => document.insert_shape(shape, None, false),
        ObjectAction::TextBox => document.insert_shape(TextBoxShape::Rectangle, None, true),
        ObjectAction::WordArt(style) => document.insert_wordart(style, "", None),
        ObjectAction::SmartArt(layout) => document.insert_smartart(layout, None),
        ObjectAction::Chart(kind) => document.insert_chart(kind, !english()),
        ObjectAction::Place(h, v) => document.place_object_on_page(need()?, h, v),
        ObjectAction::Arrange(arrange) => document.arrange_object(need()?, arrange),
        ObjectAction::Rotate(rotate) => document.rotate_object(need()?, rotate),
        ObjectAction::Effect(effect) => document.set_object_effect(need()?, effect),
        ObjectAction::Corrections(brightness, contrast) => {
            document.set_picture_corrections(need()?, brightness, contrast)
        }
        ObjectAction::Recolor(recolor) => document.recolor_picture(need()?, recolor),
        ObjectAction::CropToShape(shape) => document.crop_picture_to_shape(need()?, shape),
        ObjectAction::Reset => document.reset_picture(need()?, false),
        ObjectAction::ShapeStyle(row, column) => {
            document.apply_shape_style(need()?, ShapeStyle::gallery(row, column))
        }
    }
}

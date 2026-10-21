//! The ribbon for drawing objects: 挿入's 図 group (画像, 図形, テキスト
//! ボックス, ワードアート, SmartArt, グラフ), and the contextual tabs Word
//! shows while one is selected — 図の形式 for a picture, 図形の書式 for a
//! shape, text box or WordArt — with their 調整, スタイル, 配置 and サイズ
//! groups. Each control runs one of `jade-office-model`'s object commands.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_docx_editor::document::TextBoxShape;
use jade_docx_editor::word_document::FloatingObjectTarget;
use jade_office_model::word::objects::{
    Align, AlignTo, Arrange, ObjectSort, Rotate, WordObjects, PX_PER_MM,
};
use jade_office_model::word::picture::{Wrap, WRAPS};
use jade_office_model::word::picture_format::{
    Dash, Effect, OutlineEdit, Recolor, ReflectionPreset, ShadowPreset, WordPictureFormat,
    ARTISTIC_EFFECTS, ASPECT_RATIOS, CORRECTION_STEPS, SATURATIONS, TEMPERATURES,
};
use jade_office_model::word::shape_gallery::{crop_shapes, CATEGORIES};
use jade_office_model::word::shapes::{
    Drawn, Fill, Paint, ShapeStyle, WordArtStyle, WordShapes, STYLE_COLORS, TEXT_WARPS,
};

use crate::i18n::{self, text as t};
use crate::ribbon::{
    dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup,
};
use crate::tooltip::Tooltip;
use crate::{theme, DocumentView};

/// What the next drag on a page draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DrawTool {
    Shape(TextBoxShape),
    TextBox,
    /// A connector, with an arrowhead at its end or not.
    Connector(jade_office_model::word::connectors::ConnectorKind, bool),
    /// フリーフォーム: a click per point.
    Freeform,
    /// 描画 → ペン: one of `jade_office_model::word::ink::PENS`.
    Pen(usize),
}

/// The object tools' lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectPopup {
    /// The shape gallery, from 挿入 or 図形の書式; `true` changes the
    /// selected shape (図形の変更) rather than drawing a new one.
    Shapes(bool),
    WordArt,
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
    /// SmartArt's and charts' lists (`word_objects_diagrams`).
    Diagram(crate::word_objects_diagrams::DiagramPopup),
}

/// What an object control does.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectAction {
    Draw(DrawTool),
    InsertWordArt(WordArtStyle),
    ChangeShape(TextBoxShape),
    Corrections(f64, f64),
    Recolor(Recolor),
    Saturation(f64),
    Temperature(f64),
    Artistic(Option<&'static str>),
    Transparency(f64),
    TransparentWhite,
    RemoveBackground(bool),
    ChangePicture,
    ChangePictureFromClipboard,
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
    ToggleCrop,
    CropToShape(TextBoxShape),
    Aspect(u32, u32),
    /// サイズ: the height and width steppers, in millimetres.
    Size {
        width_mm: f64,
        height_mm: f64,
    },
    ShapeStyle(usize, usize),
    ShapeFill(Fill),
    TextFill(Paint),
    TextWarp(&'static str),
    TextAlign(&'static str),
    LayoutDialog,
    SelectionPane,
    FormatPane,
    EditWrapPoints,
    /// 頂点の編集 for the selected freeform.
    EditPoints,
    /// リンクの作成: the next text box clicked takes the selected one's
    /// overflow.
    LinkTextBox,
    /// SmartArt's and charts' controls (`word_objects_diagrams`).
    Diagram(crate::word_objects_diagrams::DiagramAction),
}

/// Colours the outline, fill and text lists offer: the Office theme's
/// accents and Word's standard colours, by name.
pub const COLORS: [(&str, &str); 14] = [
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

fn japanese() -> bool {
    i18n::language() == i18n::Language::Japanese
}

fn popup(which: ObjectPopup) -> Popup {
    Popup::Object(which)
}

fn act(action: ObjectAction) -> Action {
    Action::Object(action)
}

/// A text button that opens one of the object lists.
fn list_button(
    id: &'static str,
    label: &'static str,
    open: bool,
    which: ObjectPopup,
    enabled: bool,
    cx: &mut Context<DocumentView>,
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

/// A text button that runs an object action.
fn text_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    tooltip: &'static str,
    active: bool,
    action: ObjectAction,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
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

/// A colour square.
fn swatch(
    id: impl Into<gpui::ElementId>,
    color: &str,
    name: &'static str,
    action: ObjectAction,
    cx: &mut Context<DocumentView>,
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
    action: impl Fn(&str) -> ObjectAction,
    cx: &mut Context<DocumentView>,
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

impl DocumentView {
    /// 挿入's 図 group: 画像, 図形, テキスト ボックス, ワードアート, SmartArt,
    /// グラフ.
    pub(crate) fn insert_object_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let open = self.ribbon.popup;
        let shapes_menu = (open == Some(popup(ObjectPopup::Shapes(false))))
            .then(|| shape_gallery("shapes-menu", false, cx));
        let wordart_menu = (open == Some(popup(ObjectPopup::WordArt))).then(|| {
            menu_panel("wordart-menu")
                .w(px(240.0))
                .children(WordArtStyle::ALL.iter().enumerate().map(|(index, style)| {
                    menu_row(
                        ("wordart", index),
                        t(wordart_label(*style)),
                        false,
                        act(ObjectAction::InsertWordArt(*style)),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let insert_picture = div()
            .id("insert-picture")
            .flex()
            .flex_none()
            .items_center()
            .h(px(26.0))
            .px_2()
            .rounded_md()
            .text_xs()
            .text_color(theme::text())
            .cursor_pointer()
            .hover(|button| button.bg(theme::hovered()))
            .tooltip(Tooltip::text(t("picture.insert_title")))
            .on_click(cx.listener(|view, _event, _window, cx| view.prompt_for_picture(cx)))
            .child(t("picture.insert"))
            .into_any_element();
        let drawing = self.objects.drawing;
        let mut groups = vec![group(
            t("picture.group"),
            vec![
                row(vec![
                    insert_picture,
                    dropdown(
                        list_button(
                            "insert-shapes",
                            t("objects.shapes"),
                            open == Some(popup(ObjectPopup::Shapes(false))),
                            ObjectPopup::Shapes(false),
                            true,
                            cx,
                        ),
                        shapes_menu,
                    ),
                    text_button(
                        "insert-text-box",
                        t("objects.text_box"),
                        t("objects.text_box_title"),
                        drawing == Some(DrawTool::TextBox),
                        ObjectAction::Draw(DrawTool::TextBox),
                        cx,
                    ),
                    dropdown(
                        list_button(
                            "insert-wordart",
                            t("objects.wordart"),
                            open == Some(popup(ObjectPopup::WordArt)),
                            ObjectPopup::WordArt,
                            true,
                            cx,
                        ),
                        wordart_menu,
                    ),
                ]),
                row(self.insert_diagram_buttons(cx)),
                row(self.pen_buttons(cx)),
            ],
        )];
        if let Some(status) = self.objects.status.clone() {
            groups.push(
                div()
                    .flex_none()
                    .max_w(px(220.0))
                    .px_2()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(status)
                    .into_any_element(),
            );
        }
        groups
    }

    /// The groups of the contextual tab for the selected object.
    pub(crate) fn object_format_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(format) = self.selected_format() else {
            return vec![div()
                .px_3()
                .text_xs()
                .text_color(theme::text_faint())
                .child(t("objects.nothing_selected"))
                .into_any_element()];
        };
        let mut groups = Vec::new();
        match format.sort {
            ObjectSort::Picture => {
                groups.push(self.adjust_group(cx));
                groups.push(self.picture_style_group(cx));
            }
            ObjectSort::Shape | ObjectSort::TextBox | ObjectSort::WordArt => {
                groups.push(self.shape_insert_group(cx));
                groups.push(self.shape_style_group(cx));
                groups.push(self.wordart_group(cx));
            }
            ObjectSort::Group | ObjectSort::Chart | ObjectSort::SmartArt => {
                groups.push(self.picture_style_group(cx));
            }
        }
        groups.extend(self.diagram_groups(format.sort, cx));
        groups.push(self.arrange_group(&format, cx));
        groups.push(self.size_group(&format, cx));
        if let Some(status) = self.objects.status.clone() {
            groups.push(
                div()
                    .flex_none()
                    .max_w(px(220.0))
                    .px_2()
                    .text_xs()
                    .text_color(theme::text_dim())
                    .child(status)
                    .into_any_element(),
            );
        }
        groups
    }

    /// 図の形式 → 調整.
    fn adjust_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.ribbon.popup;
        let is_open = |which| open == Some(popup(which));
        let corrections = is_open(ObjectPopup::Corrections).then(|| {
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
                            ObjectAction::Corrections(*bright, *contrast),
                            cx,
                        )
                    }),
                ));
            }
            panel.into_any_element()
        });
        let color = is_open(ObjectPopup::Color).then(|| {
            menu_panel("color-menu")
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
                            ObjectAction::Saturation(*value),
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
                            ObjectAction::Temperature(*value),
                            cx,
                        )
                    })
                    .collect()))
                .child(menu_caption(t("objects.color.recolor")))
                .child(menu_row(
                    "recolor-none",
                    t("objects.color.none"),
                    false,
                    act(ObjectAction::Recolor(Recolor::None)),
                    cx,
                ))
                .child(menu_row(
                    "recolor-gray",
                    t("objects.color.grayscale"),
                    false,
                    act(ObjectAction::Recolor(Recolor::Grayscale)),
                    cx,
                ))
                .child(menu_row(
                    "recolor-sepia",
                    t("objects.color.sepia"),
                    false,
                    act(ObjectAction::Recolor(Recolor::Sepia)),
                    cx,
                ))
                .child(menu_row(
                    "recolor-bw",
                    t("objects.color.black_white"),
                    false,
                    act(ObjectAction::Recolor(Recolor::BlackAndWhite(0.5))),
                    cx,
                ))
                .children(
                    ["accent1", "accent2", "accent4", "accent6"]
                        .into_iter()
                        .enumerate()
                        .flat_map(|(index, slot)| {
                            [
                                menu_row(
                                    ("recolor-dark", index),
                                    i18n::format(
                                        "objects.color.accent_dark",
                                        &[("n", &slot[6..].to_string())],
                                    ),
                                    false,
                                    act(ObjectAction::Recolor(Recolor::Accent {
                                        slot: slot.to_string(),
                                        light: false,
                                    })),
                                    cx,
                                ),
                                menu_row(
                                    ("recolor-light", index),
                                    i18n::format(
                                        "objects.color.accent_light",
                                        &[("n", &slot[6..].to_string())],
                                    ),
                                    false,
                                    act(ObjectAction::Recolor(Recolor::Accent {
                                        slot: slot.to_string(),
                                        light: true,
                                    })),
                                    cx,
                                ),
                            ]
                        }),
                )
                .child(menu_row(
                    "transparent-white",
                    t("objects.color.transparent_white"),
                    false,
                    act(ObjectAction::TransparentWhite),
                    cx,
                ))
                .into_any_element()
        });
        let artistic = is_open(ObjectPopup::Artistic).then(|| {
            menu_panel("artistic-menu")
                .w(px(200.0))
                .child(menu_row(
                    "artistic-none",
                    t("objects.artistic.none"),
                    false,
                    act(ObjectAction::Artistic(None)),
                    cx,
                ))
                .children(ARTISTIC_EFFECTS.iter().enumerate().map(|(index, effect)| {
                    menu_row(
                        ("artistic", index),
                        t(artistic_label(effect)),
                        false,
                        act(ObjectAction::Artistic(Some(effect))),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let transparency = is_open(ObjectPopup::Transparency).then(|| {
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
                                act(ObjectAction::Transparency(value)),
                                cx,
                            )
                        }),
                )
                .into_any_element()
        });
        let change = is_open(ObjectPopup::ChangePicture).then(|| {
            menu_panel("change-picture-menu")
                .w(px(200.0))
                .child(menu_row(
                    "change-file",
                    t("objects.change.file"),
                    false,
                    act(ObjectAction::ChangePicture),
                    cx,
                ))
                .child(menu_row(
                    "change-clipboard",
                    t("objects.change.clipboard"),
                    false,
                    act(ObjectAction::ChangePictureFromClipboard),
                    cx,
                ))
                .into_any_element()
        });
        let reset = is_open(ObjectPopup::Reset).then(|| {
            menu_panel("reset-menu")
                .w(px(200.0))
                .child(menu_row(
                    "reset",
                    t("objects.reset"),
                    false,
                    act(ObjectAction::Reset(false)),
                    cx,
                ))
                .child(menu_row(
                    "reset-size",
                    t("objects.reset_size"),
                    false,
                    act(ObjectAction::Reset(true)),
                    cx,
                ))
                .into_any_element()
        });
        let removed = self
            .object_target()
            .and_then(|target| {
                jade_office_model::word::objects::object_run(self.document.word_document(), target)
                    .and_then(|run| run.image.as_ref())
                    .map(|image| {
                        image
                            .format()
                            .corrections
                            .as_ref()
                            .is_some_and(|corrections| corrections.background_removal.is_some())
                    })
            })
            .unwrap_or(false);
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
                        ObjectAction::RemoveBackground(!removed),
                        cx,
                    ),
                    dropdown(
                        list_button(
                            "corrections",
                            t("objects.corrections"),
                            is_open(ObjectPopup::Corrections),
                            ObjectPopup::Corrections,
                            true,
                            cx,
                        ),
                        corrections,
                    ),
                    dropdown(
                        list_button(
                            "color",
                            t("objects.color"),
                            is_open(ObjectPopup::Color),
                            ObjectPopup::Color,
                            true,
                            cx,
                        ),
                        color,
                    ),
                    dropdown(
                        list_button(
                            "artistic",
                            t("objects.artistic"),
                            is_open(ObjectPopup::Artistic),
                            ObjectPopup::Artistic,
                            true,
                            cx,
                        ),
                        artistic,
                    ),
                ]),
                row(vec![
                    dropdown(
                        list_button(
                            "transparency",
                            t("objects.transparency"),
                            is_open(ObjectPopup::Transparency),
                            ObjectPopup::Transparency,
                            true,
                            cx,
                        ),
                        transparency,
                    ),
                    dropdown(
                        list_button(
                            "change-picture",
                            t("objects.change_picture"),
                            is_open(ObjectPopup::ChangePicture),
                            ObjectPopup::ChangePicture,
                            true,
                            cx,
                        ),
                        change,
                    ),
                    dropdown(
                        list_button(
                            "reset-picture",
                            t("objects.reset"),
                            is_open(ObjectPopup::Reset),
                            ObjectPopup::Reset,
                            true,
                            cx,
                        ),
                        reset,
                    ),
                ]),
            ],
        )
    }

    /// 図のスタイル: 図の枠線 and 図の効果 (and the same for a group, chart
    /// or SmartArt's frame).
    fn picture_style_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.ribbon.popup;
        let is_open = |which| open == Some(popup(which));
        let outline =
            is_open(ObjectPopup::Outline).then(|| outline_menu("outline-menu", false, cx));
        let effects = is_open(ObjectPopup::Effects).then(|| effects_menu(cx));
        group(
            t("objects.group.picture_styles"),
            vec![
                row(vec![dropdown(
                    list_button(
                        "picture-outline",
                        t("objects.outline"),
                        is_open(ObjectPopup::Outline),
                        ObjectPopup::Outline,
                        true,
                        cx,
                    ),
                    outline,
                )]),
                row(vec![dropdown(
                    list_button(
                        "picture-effects",
                        t("objects.effects"),
                        is_open(ObjectPopup::Effects),
                        ObjectPopup::Effects,
                        true,
                        cx,
                    ),
                    effects,
                )]),
            ],
        )
    }

    /// 図形の書式 → 図形の挿入: the gallery, a text box, 図形の変更.
    fn shape_insert_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.ribbon.popup;
        let gallery = (open == Some(popup(ObjectPopup::Shapes(true))))
            .then(|| shape_gallery("change-shape-menu", true, cx));
        group(
            t("objects.group.insert_shapes"),
            vec![
                row(vec![dropdown(
                    list_button(
                        "change-shape",
                        t("objects.change_shape"),
                        open == Some(popup(ObjectPopup::Shapes(true))),
                        ObjectPopup::Shapes(true),
                        true,
                        cx,
                    ),
                    gallery,
                )]),
                row(vec![
                    text_button(
                        "edit-points",
                        t("objects.edit_points"),
                        t("objects.edit_points_title"),
                        self.objects.editing_points,
                        ObjectAction::EditPoints,
                        cx,
                    ),
                    text_button(
                        "link-text-box",
                        t("objects.link"),
                        t("objects.link_title"),
                        self.objects.linking.is_some(),
                        ObjectAction::LinkTextBox,
                        cx,
                    ),
                ]),
                row(vec![text_button(
                    "format-pane",
                    t("objects.format_pane"),
                    t("objects.format_pane_title"),
                    self.objects.panels.format_pane,
                    ObjectAction::FormatPane,
                    cx,
                )]),
            ],
        )
    }

    /// 図形のスタイル: the theme style gallery, 塗りつぶし, 枠線, 効果.
    fn shape_style_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.ribbon.popup;
        let is_open = |which| open == Some(popup(which));
        let styles = is_open(ObjectPopup::ShapeStyles).then(|| {
            let mut panel = menu_panel("shape-styles-menu").w(px(7.0 * 34.0 + 12.0));
            for style_row in 0..ShapeStyle::GALLERY_ROWS {
                panel = panel.child(div().flex().gap_1().py_0p5().children(
                    (0..STYLE_COLORS.len()).map(|column| style_swatch(style_row, column, cx)),
                ));
            }
            panel.into_any_element()
        });
        let fill =
            is_open(ObjectPopup::ShapeFill).then(|| {
                menu_panel("shape-fill-menu")
                    .w(px(180.0))
                    .child(color_row(
                        "fill-color",
                        |color| ObjectAction::ShapeFill(Fill::Paint(Paint::Rgb(color.to_string()))),
                        cx,
                    ))
                    .child(menu_row(
                        "fill-none",
                        t("objects.fill.none"),
                        false,
                        act(ObjectAction::ShapeFill(Fill::None)),
                        cx,
                    ))
                    .child(menu_caption(t("objects.transparency")))
                    .children([0.0, 0.25, 0.5, 0.75].into_iter().enumerate().map(
                        |(index, value)| {
                            menu_row(
                                ("fill-transparency", index),
                                format!("{:.0}%", value * 100.0),
                                false,
                                act(ObjectAction::ShapeFill(Fill::Transparency(value))),
                                cx,
                            )
                        },
                    ))
                    .into_any_element()
            });
        let outline =
            is_open(ObjectPopup::Outline).then(|| outline_menu("shape-outline-menu", true, cx));
        let effects = is_open(ObjectPopup::Effects).then(|| effects_menu(cx));
        group(
            t("objects.group.shape_styles"),
            vec![
                row(vec![
                    dropdown(
                        list_button(
                            "shape-styles",
                            t("objects.shape_styles"),
                            is_open(ObjectPopup::ShapeStyles),
                            ObjectPopup::ShapeStyles,
                            true,
                            cx,
                        ),
                        styles,
                    ),
                    dropdown(
                        list_button(
                            "shape-fill",
                            t("objects.fill"),
                            is_open(ObjectPopup::ShapeFill),
                            ObjectPopup::ShapeFill,
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
                            is_open(ObjectPopup::Outline),
                            ObjectPopup::Outline,
                            true,
                            cx,
                        ),
                        outline,
                    ),
                    dropdown(
                        list_button(
                            "shape-effects",
                            t("objects.shape_effects"),
                            is_open(ObjectPopup::Effects),
                            ObjectPopup::Effects,
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
    fn wordart_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.ribbon.popup;
        let is_open = |which| open == Some(popup(which));
        let text_fill = is_open(ObjectPopup::TextFill).then(|| {
            menu_panel("text-fill-menu")
                .w(px(180.0))
                .child(color_row(
                    "text-color",
                    |color| ObjectAction::TextFill(Paint::Rgb(color.to_string())),
                    cx,
                ))
                .into_any_element()
        });
        let effects = is_open(ObjectPopup::TextEffects).then(|| {
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
                        act(ObjectAction::TextWarp(preset)),
                        cx,
                    )
                }))
                .into_any_element()
        });
        let align = is_open(ObjectPopup::TextAlign).then(|| {
            menu_panel("text-align-menu")
                .w(px(140.0))
                .child(menu_row(
                    "text-top",
                    t("objects.text.top"),
                    false,
                    act(ObjectAction::TextAlign("top")),
                    cx,
                ))
                .child(menu_row(
                    "text-middle",
                    t("objects.text.middle"),
                    false,
                    act(ObjectAction::TextAlign("center")),
                    cx,
                ))
                .child(menu_row(
                    "text-bottom",
                    t("objects.text.bottom"),
                    false,
                    act(ObjectAction::TextAlign("bottom")),
                    cx,
                ))
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
                            is_open(ObjectPopup::TextFill),
                            ObjectPopup::TextFill,
                            true,
                            cx,
                        ),
                        text_fill,
                    ),
                    dropdown(
                        list_button(
                            "text-effects",
                            t("objects.text_effects"),
                            is_open(ObjectPopup::TextEffects),
                            ObjectPopup::TextEffects,
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
                        is_open(ObjectPopup::TextAlign),
                        ObjectPopup::TextAlign,
                        true,
                        cx,
                    ),
                    align,
                )]),
            ],
        )
    }

    /// 配置: 位置, 文字列の折り返し, 前面へ, 背面へ, オブジェクトの選択と
    /// 表示, 配置, グループ化, 回転.
    fn arrange_group(
        &mut self,
        format: &jade_office_model::word::objects::ObjectFormat,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.ribbon.popup;
        let is_open = |which| open == Some(popup(which));
        let floating = format.wrap != Wrap::Inline;
        let position = is_open(ObjectPopup::Position).then(|| {
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
                    act(ObjectAction::Wrap(Wrap::Inline)),
                    cx,
                ))
                .child(menu_caption(t("objects.position.with_wrapping")))
                .children(spots.into_iter().enumerate().map(|(index, (h, v, label))| {
                    menu_row(
                        ("position", index),
                        t(label),
                        false,
                        act(ObjectAction::Place(h, v)),
                        cx,
                    )
                }))
                .child(menu_row(
                    "layout-more",
                    t("objects.layout_more"),
                    false,
                    act(ObjectAction::LayoutDialog),
                    cx,
                ))
                .into_any_element()
        });
        let wrap = is_open(ObjectPopup::Wrap).then(|| {
            menu_panel("object-wrap-menu")
                .w(px(220.0))
                .children(WRAPS.iter().enumerate().map(|(index, wrap)| {
                    menu_row(
                        ("object-wrap", index),
                        crate::wrap_label(*wrap),
                        format.wrap == *wrap,
                        act(ObjectAction::Wrap(*wrap)),
                        cx,
                    )
                }))
                .when(
                    matches!(format.wrap, Wrap::Tight | Wrap::Through),
                    |panel| {
                        panel.child(menu_row(
                            "wrap-points",
                            t("objects.wrap_points"),
                            self.objects.panels.wrap_points,
                            act(ObjectAction::EditWrapPoints),
                            cx,
                        ))
                    },
                )
                .child(menu_row(
                    "wrap-more",
                    t("objects.layout_more"),
                    false,
                    act(ObjectAction::LayoutDialog),
                    cx,
                ))
                .into_any_element()
        });
        let forward = is_open(ObjectPopup::Forward).then(|| {
            menu_panel("forward-menu")
                .w(px(200.0))
                .child(menu_row(
                    "forward",
                    t("objects.forward"),
                    false,
                    act(ObjectAction::Arrange(Arrange::Forward)),
                    cx,
                ))
                .child(menu_row(
                    "to-front",
                    t("objects.to_front"),
                    false,
                    act(ObjectAction::Arrange(Arrange::ToFront)),
                    cx,
                ))
                .child(menu_row(
                    "front-of-text",
                    t("objects.in_front_of_text"),
                    false,
                    act(ObjectAction::Arrange(Arrange::InFrontOfText)),
                    cx,
                ))
                .into_any_element()
        });
        let backward = is_open(ObjectPopup::Backward).then(|| {
            menu_panel("backward-menu")
                .w(px(200.0))
                .child(menu_row(
                    "backward",
                    t("objects.backward"),
                    false,
                    act(ObjectAction::Arrange(Arrange::Backward)),
                    cx,
                ))
                .child(menu_row(
                    "to-back",
                    t("objects.to_back"),
                    false,
                    act(ObjectAction::Arrange(Arrange::ToBack)),
                    cx,
                ))
                .child(menu_row(
                    "behind-text",
                    t("objects.behind_text"),
                    false,
                    act(ObjectAction::Arrange(Arrange::BehindText)),
                    cx,
                ))
                .into_any_element()
        });
        let align_to = self.objects.panels.align_to;
        let align = is_open(ObjectPopup::Align).then(|| {
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
                                act(ObjectAction::Align(align)),
                                cx,
                            )
                        }),
                )
                .child(menu_caption(t("objects.align.relative")))
                .child(menu_row(
                    "align-page",
                    t("objects.align.page"),
                    align_to == AlignTo::Page,
                    act(ObjectAction::AlignTo(AlignTo::Page)),
                    cx,
                ))
                .child(menu_row(
                    "align-margin",
                    t("objects.align.margin"),
                    align_to == AlignTo::Margin,
                    act(ObjectAction::AlignTo(AlignTo::Margin)),
                    cx,
                ))
                .child(menu_row(
                    "align-selection",
                    t("objects.align.selection"),
                    align_to == AlignTo::Selection,
                    act(ObjectAction::AlignTo(AlignTo::Selection)),
                    cx,
                ))
                .into_any_element()
        });
        let group_menu = is_open(ObjectPopup::Group).then(|| {
            menu_panel("group-menu")
                .w(px(180.0))
                .child(menu_row(
                    "group",
                    t("objects.group"),
                    false,
                    act(ObjectAction::Group(true)),
                    cx,
                ))
                .child(menu_row(
                    "ungroup",
                    t("objects.ungroup"),
                    false,
                    act(ObjectAction::Group(false)),
                    cx,
                ))
                .into_any_element()
        });
        let rotate = is_open(ObjectPopup::Rotate).then(|| {
            menu_panel("rotate-menu")
                .w(px(220.0))
                .child(menu_row(
                    "rotate-right",
                    t("objects.rotate.right"),
                    false,
                    act(ObjectAction::Rotate(Rotate::Right90)),
                    cx,
                ))
                .child(menu_row(
                    "rotate-left",
                    t("objects.rotate.left"),
                    false,
                    act(ObjectAction::Rotate(Rotate::Left90)),
                    cx,
                ))
                .child(menu_row(
                    "flip-vertical",
                    t("objects.rotate.flip_v"),
                    format.flip_vertical,
                    act(ObjectAction::Rotate(Rotate::FlipVertical)),
                    cx,
                ))
                .child(menu_row(
                    "flip-horizontal",
                    t("objects.rotate.flip_h"),
                    format.flip_horizontal,
                    act(ObjectAction::Rotate(Rotate::FlipHorizontal)),
                    cx,
                ))
                .child(menu_row(
                    "rotate-more",
                    t("objects.rotate.more"),
                    false,
                    act(ObjectAction::LayoutDialog),
                    cx,
                ))
                .into_any_element()
        });
        group(
            t("objects.group.arrange"),
            vec![
                row(vec![
                    dropdown(
                        list_button(
                            "object-position",
                            t("objects.position"),
                            is_open(ObjectPopup::Position),
                            ObjectPopup::Position,
                            true,
                            cx,
                        ),
                        position,
                    ),
                    dropdown(
                        list_button(
                            "object-wrap",
                            t("picture.wrap"),
                            is_open(ObjectPopup::Wrap),
                            ObjectPopup::Wrap,
                            true,
                            cx,
                        ),
                        wrap,
                    ),
                    dropdown(
                        list_button(
                            "bring-forward",
                            t("objects.forward"),
                            is_open(ObjectPopup::Forward),
                            ObjectPopup::Forward,
                            floating,
                            cx,
                        ),
                        forward,
                    ),
                    dropdown(
                        list_button(
                            "send-backward",
                            t("objects.backward"),
                            is_open(ObjectPopup::Backward),
                            ObjectPopup::Backward,
                            floating,
                            cx,
                        ),
                        backward,
                    ),
                ]),
                row(vec![
                    text_button(
                        "selection-pane",
                        t("objects.selection_pane"),
                        t("objects.selection_pane_title"),
                        self.objects.panels.selection_pane,
                        ObjectAction::SelectionPane,
                        cx,
                    ),
                    dropdown(
                        list_button(
                            "align-objects",
                            t("objects.align"),
                            is_open(ObjectPopup::Align),
                            ObjectPopup::Align,
                            floating,
                            cx,
                        ),
                        align,
                    ),
                    dropdown(
                        list_button(
                            "group-objects",
                            t("objects.group"),
                            is_open(ObjectPopup::Group),
                            ObjectPopup::Group,
                            true,
                            cx,
                        ),
                        group_menu,
                    ),
                    dropdown(
                        list_button(
                            "rotate-objects",
                            t("objects.rotate"),
                            is_open(ObjectPopup::Rotate),
                            ObjectPopup::Rotate,
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
        &mut self,
        format: &jade_office_model::word::objects::ObjectFormat,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.ribbon.popup;
        let is_open = |which| open == Some(popup(which));
        let picture = format.sort == ObjectSort::Picture;
        let crop = is_open(ObjectPopup::Crop).then(|| {
            menu_panel("crop-menu")
                .id("crop-menu")
                .w(px(260.0))
                .max_h(px(440.0))
                .overflow_y_scroll()
                .child(menu_row(
                    "crop",
                    t("objects.crop"),
                    self.objects.cropping,
                    act(ObjectAction::ToggleCrop),
                    cx,
                ))
                .child(menu_caption(t("objects.crop.aspect")))
                .children(ASPECT_RATIOS.iter().enumerate().map(|(index, (w, h))| {
                    menu_row(
                        ("aspect", index),
                        format!("{w}:{h}"),
                        false,
                        act(ObjectAction::Aspect(*w, *h)),
                        cx,
                    )
                }))
                .child(menu_caption(t("objects.crop.shape")))
                .children(crop_shapes().enumerate().map(|(index, (shape, ja, en))| {
                    menu_row(
                        ("crop-shape", index),
                        if japanese() { ja } else { en },
                        false,
                        act(ObjectAction::CropToShape(shape)),
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
                    ObjectAction::Size {
                        width_mm: (value + delta).max(1.0),
                        height_mm,
                    }
                } else {
                    ObjectAction::Size {
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
                row(vec![dropdown(
                    list_button(
                        "crop-picture",
                        t("objects.crop"),
                        is_open(ObjectPopup::Crop),
                        ObjectPopup::Crop,
                        picture,
                        cx,
                    ),
                    crop,
                )]),
                stepper("height", t("objects.size.height"), height_mm, false, cx),
                stepper("width", t("objects.size.width"), width_mm, true, cx),
            ],
        )
    }

    /// Runs an object control.
    pub(crate) fn run_object_action(&mut self, action: ObjectAction, cx: &mut Context<Self>) {
        self.ribbon.popup = None;
        if let ObjectAction::Draw(tool) = action {
            // Picked again: put the tool down.
            self.objects.drawing = (self.objects.drawing != Some(tool)).then_some(tool);
            self.objects.status = self
                .objects
                .drawing
                .map(|_| SharedString::from(t("objects.draw_hint")));
            return cx.notify();
        }
        if let ObjectAction::Diagram(action) = action {
            return self.run_diagram_action(action, cx);
        }
        if let ObjectAction::InsertWordArt(style) = action {
            self.place_document_caret();
            return self.object_edit(cx, |view| view.document.insert_wordart(style, "", None));
        }
        match action {
            ObjectAction::LayoutDialog => return self.open_layout_dialog(cx),
            ObjectAction::SelectionPane => {
                self.objects.panels.selection_pane = !self.objects.panels.selection_pane;
                return cx.notify();
            }
            ObjectAction::FormatPane => {
                self.objects.panels.format_pane = !self.objects.panels.format_pane;
                return cx.notify();
            }
            ObjectAction::EditWrapPoints => {
                self.objects.panels.wrap_points = !self.objects.panels.wrap_points;
                return cx.notify();
            }
            ObjectAction::EditPoints => {
                self.objects.editing_points = !self.objects.editing_points;
                return cx.notify();
            }
            ObjectAction::LinkTextBox => {
                self.objects.linking = match self.objects.linking {
                    Some(_) => None,
                    None => self.object_target(),
                };
                self.objects.status = self
                    .objects
                    .linking
                    .map(|_| SharedString::from(t("objects.link_hint")));
                return cx.notify();
            }
            ObjectAction::AlignTo(to) => {
                self.objects.panels.align_to = to;
                return cx.notify();
            }
            ObjectAction::ToggleCrop => {
                self.objects.cropping = !self.objects.cropping;
                return cx.notify();
            }
            ObjectAction::ChangePicture => return self.prompt_to_change_picture(cx),
            ObjectAction::ChangePictureFromClipboard => {
                return self.change_picture_from_clipboard(cx)
            }
            ObjectAction::Align(align) => {
                let objects: Vec<_> = self
                    .selected_objects()
                    .iter()
                    .map(crate::word_objects::placed)
                    .collect();
                let to = self.objects.panels.align_to;
                return self
                    .object_edit(cx, |view| view.document.align_objects(&objects, align, to));
            }
            ObjectAction::Group(group) => {
                let objects: Vec<_> = self
                    .selected_objects()
                    .iter()
                    .map(crate::word_objects::placed)
                    .collect();
                return self.object_edit(cx, |view| {
                    if group {
                        view.group_selected(objects)
                    } else {
                        view.ungroup_selected()
                    }
                });
            }
            _ => {}
        }
        let Some(target) = self.object_target() else {
            return;
        };
        self.object_edit(cx, |view| run_on(view, target, action));
        if self.objects.cropping {
            self.refresh_selection();
        }
    }

    /// Puts the document's selection where the view's caret is, for a
    /// command that inserts there.
    pub(crate) fn place_document_caret(&mut self) {
        self.document
            .place_caret(self.insert_at.0, self.insert_at.1);
    }

    /// The shape tool's drag ended: the shape, text box or WordArt goes in
    /// where it was drawn, anchored at the paragraph there.
    pub(crate) fn draw_shape(
        &mut self,
        tool: DrawTool,
        page_index: usize,
        (x0, y0, x1, y1): (f64, f64, f64, f64),
        cx: &mut Context<Self>,
    ) {
        if let DrawTool::Connector(kind, arrow) = tool {
            return self.draw_connector(kind, arrow, page_index, (x0, y0), (x1, y1), cx);
        }
        let (x, y) = (x0.min(x1), y0.min(y1));
        let (width, height) = ((x1 - x0).abs(), (y1 - y0).abs());
        // A click rather than a drag puts it in at its default size there.
        let drawn = if width < 4.0 && height < 4.0 {
            let (w, h) = match tool {
                DrawTool::TextBox => jade_office_model::word::shapes::DEFAULT_TEXT_BOX,
                _ => jade_office_model::word::shapes::DEFAULT_SHAPE,
            };
            Drawn {
                x,
                y,
                width: w,
                height: h,
            }
        } else {
            Drawn {
                x,
                y,
                width,
                height,
            }
        };
        let document_y = self.document.document_y(page_index, y);
        if let Some(position) = self.document.position_at(x, document_y) {
            self.insert_at = position;
        }
        self.place_document_caret();
        self.objects.status = None;
        self.object_edit(cx, |view| match tool {
            DrawTool::Shape(shape) => view.document.insert_shape(shape, Some(drawn), false),
            DrawTool::TextBox => {
                view.document
                    .insert_shape(TextBoxShape::Rectangle, Some(drawn), true)
            }
            DrawTool::Connector(..) | DrawTool::Freeform | DrawTool::Pen(_) => Ok(()),
        });
        // Select what went in: the object drawn at that spot.
        let point_y = self
            .document
            .document_y(page_index, drawn.y + drawn.height / 2.0);
        if let Some(object) = self
            .document
            .object_at(drawn.x + drawn.width / 2.0, point_y)
        {
            self.selected_object = Some(object);
            self.ribbon_tab = crate::RibbonTab::ObjectFormat;
        }
        cx.notify();
    }
}

/// Runs one object control on `target`.
fn run_on(
    view: &mut DocumentView,
    target: FloatingObjectTarget,
    action: ObjectAction,
) -> Result<(), String> {
    let document = &mut view.document;
    match action {
        ObjectAction::ChangeShape(shape) => document.change_shape(target, shape),
        ObjectAction::Corrections(brightness, contrast) => {
            document.set_picture_corrections(target, brightness, contrast)
        }
        ObjectAction::Recolor(recolor) => document.recolor_picture(target, recolor),
        ObjectAction::Saturation(value) => document.set_picture_saturation(target, value),
        ObjectAction::Temperature(value) => document.set_picture_temperature(target, value),
        ObjectAction::Artistic(effect) => document.set_artistic_effect(target, effect),
        ObjectAction::Transparency(value) => document.set_picture_transparency(target, value),
        ObjectAction::TransparentWhite => document.set_transparent_color(target, "FFFFFF"),
        ObjectAction::RemoveBackground(remove) => {
            document.remove_picture_background(target, remove)
        }
        ObjectAction::Reset(size) => document.reset_picture(target, size),
        ObjectAction::Outline(edit) => document.set_picture_outline(target, edit),
        ObjectAction::Effect(effect) => document.set_object_effect(target, effect),
        ObjectAction::Place(h, v) => document.place_object_on_page(target, h, v),
        ObjectAction::Wrap(wrap) => document.set_object_wrap(target, wrap),
        ObjectAction::Arrange(arrange) => document.arrange_object(target, arrange),
        ObjectAction::Rotate(rotate) => document.rotate_object(target, rotate),
        ObjectAction::CropToShape(shape) => document.crop_picture_to_shape(target, shape),
        ObjectAction::Aspect(width, height) => {
            document.crop_picture_to_aspect(target, (width, height))
        }
        ObjectAction::Size {
            width_mm,
            height_mm,
        } => document.resize_object(target, width_mm * PX_PER_MM, height_mm * PX_PER_MM),
        ObjectAction::ShapeStyle(row, column) => {
            document.apply_shape_style(target, ShapeStyle::gallery(row, column))
        }
        ObjectAction::ShapeFill(fill) => document.set_shape_fill(target, fill),
        ObjectAction::TextFill(paint) => document.set_shape_text_color(target, paint),
        ObjectAction::TextWarp(preset) => document.set_text_warp(target, preset),
        ObjectAction::TextAlign(align) => {
            use jade_docx_editor::commands::format_text_box::{
                FormatTextBoxCommand, TextBoxFormatSpec,
            };
            use jade_docx_editor::document::TextBoxVerticalAlignment;
            use jade_office_model::word::WordSession;
            let alignment = match align {
                "top" => TextBoxVerticalAlignment::Top,
                "bottom" => TextBoxVerticalAlignment::Bottom,
                _ => TextBoxVerticalAlignment::Center,
            };
            WordSession::execute_command(
                document,
                Box::new(FormatTextBoxCommand::new(
                    target,
                    TextBoxFormatSpec {
                        text_vertical_align: Some(alignment),
                        ..Default::default()
                    },
                )),
                true,
            )
        }
        _ => Ok(()),
    }
}

/// The shape gallery, by category; `change` changes the selected shape.
fn shape_gallery(id: &'static str, change: bool, cx: &mut Context<DocumentView>) -> AnyElement {
    let mut panel = menu_panel(id)
        .w(px(300.0))
        .max_h(px(460.0))
        .overflow_y_scroll();
    let mut index = 0usize;
    for category in CATEGORIES.iter() {
        if change && category.shapes.iter().all(|(shape, _, _)| shape.is_line()) {
            continue;
        }
        panel = panel.child(menu_caption(if japanese() {
            category.ja
        } else {
            category.en
        }));
        for (shape, ja, en) in category.shapes {
            use jade_office_model::word::connectors::ConnectorKind;
            let action = if change {
                ObjectAction::ChangeShape(*shape)
            } else {
                // The 線 gallery draws connectors, which glue to shapes.
                ObjectAction::Draw(match shape {
                    TextBoxShape::Line => DrawTool::Connector(ConnectorKind::Straight, false),
                    TextBoxShape::LineArrow | TextBoxShape::LineArrowDouble => {
                        DrawTool::Connector(ConnectorKind::Straight, true)
                    }
                    TextBoxShape::LineBent => DrawTool::Connector(ConnectorKind::Elbow, true),
                    TextBoxShape::LineCurved => DrawTool::Connector(ConnectorKind::Curved, true),
                    other => DrawTool::Shape(*other),
                })
            };
            panel = panel.child(menu_row(
                (id, index),
                if japanese() { *ja } else { *en },
                false,
                act(action),
                cx,
            ));
            index += 1;
        }
        if !change && category.en == "Lines" {
            panel = panel.child(menu_row(
                (id, 999usize),
                t("objects.freeform"),
                false,
                act(ObjectAction::Draw(DrawTool::Freeform)),
                cx,
            ));
        }
    }
    panel.into_any_element()
}

/// 図の枠線 / 図形の枠線: colour, none, weight, dashes (and arrows for a
/// shape).
fn outline_menu(id: &'static str, arrows: bool, cx: &mut Context<DocumentView>) -> AnyElement {
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
    menu_panel(id)
        .id(id)
        .w(px(200.0))
        .max_h(px(460.0))
        .overflow_y_scroll()
        .child(color_row(
            "outline-color",
            |color| ObjectAction::Outline(OutlineEdit::Color(color.to_string())),
            cx,
        ))
        .child(menu_row(
            "outline-none",
            t("objects.outline.none"),
            false,
            act(ObjectAction::Outline(OutlineEdit::None)),
            cx,
        ))
        .child(menu_caption(t("objects.outline.weight")))
        .children(weights.into_iter().enumerate().map(|(index, weight)| {
            menu_row(
                ("outline-weight", index),
                format!("{weight} pt"),
                false,
                act(ObjectAction::Outline(OutlineEdit::Width(weight))),
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
                        act(ObjectAction::Outline(OutlineEdit::Dash(dash))),
                        cx,
                    )
                }),
        )
        .when(arrows, |panel| {
            panel
                .child(menu_caption(t("objects.outline.arrows")))
                .child(menu_row(
                    "arrow-none",
                    t("objects.arrow.none"),
                    false,
                    act(ObjectAction::Outline(OutlineEdit::Arrows(None, None))),
                    cx,
                ))
                .child(menu_row(
                    "arrow-end",
                    t("objects.arrow.end"),
                    false,
                    act(ObjectAction::Outline(OutlineEdit::Arrows(
                        None,
                        Some("triangle".into()),
                    ))),
                    cx,
                ))
                .child(menu_row(
                    "arrow-start",
                    t("objects.arrow.start"),
                    false,
                    act(ObjectAction::Outline(OutlineEdit::Arrows(
                        Some("triangle".into()),
                        None,
                    ))),
                    cx,
                ))
                .child(menu_row(
                    "arrow-both",
                    t("objects.arrow.both"),
                    false,
                    act(ObjectAction::Outline(OutlineEdit::Arrows(
                        Some("triangle".into()),
                        Some("triangle".into()),
                    ))),
                    cx,
                ))
        })
        .into_any_element()
}

/// 図の効果 / 図形の効果: 影, 反射, 光彩, ぼかし, 面取り.
fn effects_menu(cx: &mut Context<DocumentView>) -> AnyElement {
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
    menu_panel("effects-menu")
        .id("effects-menu")
        .w(px(240.0))
        .max_h(px(480.0))
        .overflow_y_scroll()
        .child(menu_caption(t("objects.effect.shadow")))
        .child(menu_row(
            "shadow-none",
            t("objects.effect.none"),
            false,
            act(ObjectAction::Effect(Effect::Shadow(None))),
            cx,
        ))
        .children(ShadowPreset::OUTER.iter().zip(shadows).enumerate().map(
            |(index, (preset, label))| {
                menu_row(
                    ("shadow", index),
                    t(label),
                    false,
                    act(ObjectAction::Effect(Effect::Shadow(Some(*preset)))),
                    cx,
                )
            },
        ))
        .children(ShadowPreset::INNER.iter().zip(inner).enumerate().map(
            |(index, (preset, label))| {
                menu_row(
                    ("inner-shadow", index),
                    t(label),
                    false,
                    act(ObjectAction::Effect(Effect::Shadow(Some(*preset)))),
                    cx,
                )
            },
        ))
        .child(menu_caption(t("objects.effect.reflection")))
        .child(menu_row(
            "reflection-none",
            t("objects.effect.none"),
            false,
            act(ObjectAction::Effect(Effect::Reflection(None))),
            cx,
        ))
        .child(menu_row(
            "reflection-tight",
            t("objects.reflection.tight"),
            false,
            act(ObjectAction::Effect(Effect::Reflection(Some(
                ReflectionPreset::Tight,
            )))),
            cx,
        ))
        .child(menu_row(
            "reflection-half",
            t("objects.reflection.half"),
            false,
            act(ObjectAction::Effect(Effect::Reflection(Some(
                ReflectionPreset::Half,
            )))),
            cx,
        ))
        .child(menu_row(
            "reflection-full",
            t("objects.reflection.full"),
            false,
            act(ObjectAction::Effect(Effect::Reflection(Some(
                ReflectionPreset::Full,
            )))),
            cx,
        ))
        .child(menu_caption(t("objects.effect.glow")))
        .child(menu_row(
            "glow-none",
            t("objects.effect.none"),
            false,
            act(ObjectAction::Effect(Effect::Glow(None))),
            cx,
        ))
        .children(
            [
                (5.0, "4472C4"),
                (8.0, "ED7D31"),
                (11.0, "70AD47"),
                (18.0, "FFC000"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (radius, color))| {
                menu_row(
                    ("glow", index),
                    i18n::format("objects.glow.size", &[("n", &radius)]),
                    false,
                    act(ObjectAction::Effect(Effect::Glow(Some((
                        radius,
                        color.to_string(),
                    ))))),
                    cx,
                )
            }),
        )
        .child(menu_caption(t("objects.effect.soft_edges")))
        .child(menu_row(
            "soft-none",
            t("objects.effect.none"),
            false,
            act(ObjectAction::Effect(Effect::SoftEdge(None))),
            cx,
        ))
        .children(
            [1.0, 2.5, 5.0, 10.0, 25.0, 50.0]
                .into_iter()
                .enumerate()
                .map(|(index, radius)| {
                    menu_row(
                        ("soft", index),
                        format!("{radius} pt"),
                        false,
                        act(ObjectAction::Effect(Effect::SoftEdge(Some(radius)))),
                        cx,
                    )
                }),
        )
        .child(menu_caption(t("objects.effect.bevel")))
        .child(menu_row(
            "bevel-none",
            t("objects.effect.none"),
            false,
            act(ObjectAction::Effect(Effect::Bevel(None))),
            cx,
        ))
        .children(
            [
                ("circle", "objects.bevel.circle"),
                ("relaxedInset", "objects.bevel.relaxed_inset"),
                ("cross", "objects.bevel.cross"),
                ("coolSlant", "objects.bevel.cool_slant"),
                ("angle", "objects.bevel.angle"),
                ("softRound", "objects.bevel.soft_round"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (preset, label))| {
                menu_row(
                    ("bevel", index),
                    t(label),
                    false,
                    act(ObjectAction::Effect(Effect::Bevel(Some(
                        preset.to_string(),
                    )))),
                    cx,
                )
            }),
        )
        .into_any_element()
}

/// One square of the style gallery, drawn in its fill and outline.
fn style_swatch(row: usize, column: usize, cx: &mut Context<DocumentView>) -> AnyElement {
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
        .id(("shape-style", row * 10 + column))
        .size(px(30.0))
        .rounded_sm()
        .bg(gpui::rgb(fill))
        .border_2()
        .border_color(gpui::rgb(line))
        .cursor_pointer()
        .hover(|square| square.border_color(theme::accent()))
        .on_click(listener(act(ObjectAction::ShapeStyle(row, column)), cx))
        .into_any_element()
}

fn wordart_label(style: WordArtStyle) -> &'static str {
    match style {
        WordArtStyle::BlackShadow => "objects.wordart.black_shadow",
        WordArtStyle::AccentShadow => "objects.wordart.accent_shadow",
        WordArtStyle::WhiteOutlineGlow => "objects.wordart.white_outline_glow",
        WordArtStyle::OutlineOnly => "objects.wordart.outline_only",
        WordArtStyle::OrangeOutline => "objects.wordart.orange_outline",
        WordArtStyle::GrayBevel => "objects.wordart.gray_bevel",
    }
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

impl DocumentView {
    /// A connector's drag ended: each end glued to the shape it is on, at
    /// the connection point nearest it.
    fn draw_connector(
        &mut self,
        kind: jade_office_model::word::connectors::ConnectorKind,
        arrow: bool,
        page_index: usize,
        start: (f64, f64),
        end: (f64, f64),
        cx: &mut Context<Self>,
    ) {
        use jade_office_model::word::connectors::{
            nearest_connection, ConnectorEnd, WordConnectors,
        };
        let end = if (end.0 - start.0).abs() + (end.1 - start.1).abs() < 4.0 {
            (start.0 + 96.0, start.1)
        } else {
            end
        };
        let glue = |view: &Self, point: (f64, f64)| {
            let document_y = view.document.document_y(page_index, point.1);
            view.document
                .object_at(point.0, document_y)
                .filter(|object| {
                    object.kind == jade_docx_editor::word_document::FloatingObjectKind::TextBox
                })
                .map(|object| {
                    let placed = crate::word_objects::placed(&object);
                    (
                        crate::word_objects::target_of(&object),
                        nearest_connection(&placed, point),
                    )
                })
        };
        let first = ConnectorEnd {
            point: start,
            glued: glue(self, start),
        };
        let last = ConnectorEnd {
            point: end,
            glued: glue(self, end),
        };
        // Glued ends start and stop at the connection points.
        let snap = |view: &Self, end: ConnectorEnd| match end.glued {
            Some((target, index)) => crate::word_objects::region_hit(&view.document, target)
                .map(|object| {
                    jade_office_model::word::connectors::connection_point(
                        &crate::word_objects::placed(&object),
                        index,
                    )
                })
                .map(|point| ConnectorEnd { point, ..end })
                .unwrap_or(end),
            None => end,
        };
        let (first, last) = (snap(self, first), snap(self, last));
        let document_y = self.document.document_y(page_index, start.1);
        if let Some(position) = self.document.position_at(start.0, document_y) {
            self.insert_at = position;
        }
        self.place_document_caret();
        self.object_edit(cx, |view| {
            view.document.insert_connector(kind, arrow, first, last)
        });
        cx.notify();
    }

    /// After objects moved: the connectors glued to them follow.
    pub(crate) fn reroute_connectors(&mut self) {
        use jade_office_model::word::connectors::WordConnectors;
        let regions: Vec<_> = self
            .document
            .body_objects()
            .iter()
            .filter_map(|object| crate::word_objects::region_hit(&self.document, object.target))
            .map(|object| crate::word_objects::placed(&object))
            .collect();
        match self.document.reroute_connectors(&regions) {
            Ok(true) => {
                self.rebuild_after_edit();
                self.refresh_selection();
            }
            Ok(false) => {}
            Err(error) => self.objects.status = Some(error.into()),
        }
    }
}

impl DocumentView {
    /// 描画's pens: a drag on the page draws a stroke, put in as Word ink
    /// when the mouse comes up.
    fn pen_buttons(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let drawing = self.objects.drawing;
        jade_office_model::word::ink::PENS
            .iter()
            .enumerate()
            .map(|(index, (color, _, _, ja, en))| {
                let value = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0);
                let active = drawing == Some(DrawTool::Pen(index));
                div()
                    .id(("pen", index))
                    .flex()
                    .items_center()
                    .gap_1()
                    .h(px(26.0))
                    .px_2()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .when(active, |button| button.bg(theme::selected()))
                    .hover(|button| button.bg(theme::hovered()))
                    .tooltip(Tooltip::text(if japanese() { *ja } else { *en }))
                    .child(
                        div()
                            .size(px(10.0))
                            .rounded_full()
                            .bg(gpui::rgb(value))
                            .border_1()
                            .border_color(theme::border()),
                    )
                    .child(t("objects.pen"))
                    .on_click(listener(act(ObjectAction::Draw(DrawTool::Pen(index))), cx))
                    .into_any_element()
            })
            .collect()
    }

    /// A pen stroke's drag ended: it goes in as ink.
    pub(crate) fn finish_stroke(&mut self, cx: &mut Context<Self>) {
        use jade_office_model::word::ink::WordInk;
        let Some(DrawTool::Pen(index)) = self.objects.drawing else {
            return;
        };
        let Some((page_index, points)) = self.objects.stroke.take() else {
            return;
        };
        let (color, width, highlighter, _, _) = jade_office_model::word::ink::PENS[index];
        let stroke = jade_docx_editor::document::InkStroke {
            points: points.clone(),
            color: color.to_string(),
            width,
            highlighter,
        };
        let document_y = self.document.document_y(page_index, points[0].1);
        if let Some(position) = self.document.position_at(points[0].0, document_y) {
            self.insert_at = position;
        }
        self.place_document_caret();
        self.object_edit(cx, |view| view.document.insert_ink(vec![stroke]));
        // The pen stays in hand for the next stroke.
        self.objects.drawing = Some(DrawTool::Pen(index));
    }
}

//! The browser's share of the Word tools the native app has
//! (`jade-gpui-app`'s `word_tools` and its modules), over the same
//! `jade-office-model` calls: the 箇条書き, 段落番号 and アウトライン
//! libraries — Japanese number formats included — with 新しいリストを開始,
//! 前のリストから継続 and the list level; 均等割り付け and the 段落 switches;
//! ドロップキャップ; and the デザイン tab — テーマ, 配色, フォント, 効果, the
//! style sets, 段落の間隔 and 既定に設定. The dialogs and the rulers are the
//! native app's for now.

use gpui::{div, prelude::*, px, rgb, AnyElement, Context};
use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::design::{
    style_sets, DesignState, WordDesign, COLOR_SCHEMES, EFFECT_SCHEMES, FONT_SCHEMES,
    PARAGRAPH_SPACINGS, THEMES,
};
use jade_web::word::lists::{
    ListLevel, ListState, MultilevelPreset, WordLists, BULLETS, NUMBER_FORMATS,
};
use jade_web::word::paragraph_options::{
    DropCapPosition, ParagraphOptionsState, ParagraphSwitch, WordParagraphOptions,
};
use jade_web::word::styles::WordStyles;

use crate::ribbon::{
    dropdown, group, menu_caption, menu_panel, menu_row, row, Action, Popup, Ribbon,
};
use crate::view::DocView;
use crate::{i18n, palette};

/// The tools' lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMenu {
    Bullets,
    Numbering,
    Multilevel,
    ParagraphSettings,
    DropCap,
    Themes,
    Colors,
    Fonts,
    Effects,
    StyleSets,
    Spacing,
}

/// What a tool's entry does.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolCommand {
    Switch(ParagraphSwitch, bool),
    DropCap(Option<(DropCapPosition, u8)>),
    NumberFormat(usize),
    Bullet(usize),
    Multilevel(MultilevelPreset),
    ListLevel(u8),
    RestartList,
    ContinueList,
    Theme(usize),
    Colors(usize),
    Fonts(usize),
    Effects(usize),
    StyleSet(usize),
    Spacing(usize),
    SetAsDefault,
}

/// What the tools' open list shows as current, read only while one is.
#[derive(Default)]
pub struct ToolState {
    pub list: Option<ListState>,
    pub paragraph: ParagraphOptionsState,
    pub design: DesignState,
}

impl ToolState {
    pub fn read(document: &EditableDocument, open: Option<ToolMenu>) -> Self {
        let Some(open) = open else {
            return ToolState::default();
        };
        match open {
            ToolMenu::Bullets | ToolMenu::Numbering | ToolMenu::Multilevel => ToolState {
                list: document.list_state(),
                ..ToolState::default()
            },
            ToolMenu::ParagraphSettings | ToolMenu::DropCap => ToolState {
                paragraph: document.paragraph_options(),
                ..ToolState::default()
            },
            _ => ToolState {
                design: document.design_state(),
                ..ToolState::default()
            },
        }
    }
}

/// Runs a tool's command on the document.
pub fn run(document: &mut EditableDocument, command: ToolCommand) -> Result<(), String> {
    match command {
        ToolCommand::Switch(switch, on) => document.set_paragraph_switch(switch, on),
        ToolCommand::DropCap(drop_cap) => document.set_drop_cap(drop_cap),
        ToolCommand::NumberFormat(index) => {
            let format = NUMBER_FORMATS[index];
            document.apply_list_format(format.format, format.text, None)
        }
        ToolCommand::Bullet(index) => {
            let (glyph, font) = BULLETS[index];
            document.apply_list_format("bullet", glyph, font)
        }
        ToolCommand::Multilevel(preset) => document.apply_multilevel_preset(preset),
        ToolCommand::ListLevel(level) => document.set_list_level(level),
        ToolCommand::RestartList => document.set_list_start(1),
        ToolCommand::ContinueList => document.continue_list(),
        ToolCommand::Theme(index) => document.apply_theme(&THEMES[index]),
        ToolCommand::Colors(index) => document.apply_color_scheme(&COLOR_SCHEMES[index]),
        ToolCommand::Fonts(index) => document.apply_font_scheme(&FONT_SCHEMES[index]),
        ToolCommand::Effects(index) => document.apply_effect_scheme(&EFFECT_SCHEMES[index]),
        ToolCommand::StyleSet(index) => document.apply_style_set(&style_sets()[index]),
        ToolCommand::Spacing(index) => {
            let (_, _, before, after, line) = PARAGRAPH_SPACINGS[index];
            document.apply_paragraph_spacing(before, after, line)
        }
        ToolCommand::SetAsDefault => document.set_as_default(),
    }
}

/// Applies a gallery style the way Word does: character and linked styles
/// as such.
pub fn apply_style(document: &mut EditableDocument, id: &str) -> Result<(), String> {
    document.apply_style(id)
}

fn text_button(
    id: &'static str,
    label: &'static str,
    open: bool,
    menu: ToolMenu,
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
        .text_color(palette::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(open, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .on_click(crate::ribbon::listener(
            Action::Popup(Popup::Word(menu)),
            cx,
        ))
        .child(i18n::text(label))
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(palette::dim()),
        )
        .into_any_element()
}

fn command_row(
    id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
    checked: bool,
    command: ToolCommand,
    cx: &mut Context<DocView>,
) -> AnyElement {
    menu_row(id, label, checked, Action::Word(command), cx)
}

/// ホーム → リスト: the libraries, 均等割り付け and 段落's settings.
pub fn list_group(
    ribbon: &Ribbon,
    tools: &ToolState,
    alignment: Option<&str>,
    cx: &mut Context<DocView>,
) -> AnyElement {
    let open = |menu| ribbon.popup == Some(Popup::Word(menu));
    let bullets = open(ToolMenu::Bullets).then(|| {
        menu_panel("bullets-menu")
            .w(px(200.0))
            .child(menu_caption(i18n::text("word.list.bullet_library")))
            .children(BULLETS.iter().enumerate().map(|(index, (glyph, _))| {
                command_row(
                    ("bullet", index),
                    *glyph,
                    false,
                    ToolCommand::Bullet(index),
                    cx,
                )
            }))
            .into_any_element()
    });
    let numbering = open(ToolMenu::Numbering).then(|| {
        let mut panel = menu_panel("numbering-menu")
            .w(px(220.0))
            .max_h(px(360.0))
            .overflow_y_scroll()
            .child(menu_caption(i18n::text("word.list.number_library")))
            .children(NUMBER_FORMATS.iter().enumerate().map(|(index, format)| {
                let level = ListLevel::numbered(0, format.format, format.text);
                let formats = [format.format.to_string()];
                let sample = format!(
                    "{}  {}",
                    level.preview(&[1], &formats),
                    level.preview(&[2], &formats)
                );
                command_row(
                    ("number-format", index),
                    sample,
                    false,
                    ToolCommand::NumberFormat(index),
                    cx,
                )
            }));
        if tools.list.is_some() {
            panel = panel
                .child(command_row(
                    "list-restart",
                    i18n::text("word.list.restart"),
                    false,
                    ToolCommand::RestartList,
                    cx,
                ))
                .child(command_row(
                    "list-continue",
                    i18n::text("word.list.continue"),
                    false,
                    ToolCommand::ContinueList,
                    cx,
                ));
        }
        panel.into_any_element()
    });
    let multilevel = open(ToolMenu::Multilevel).then(|| {
        let labels = [
            (MultilevelPreset::DecimalOutline, "word.list.preset.outline"),
            (MultilevelPreset::Parenthesised, "word.list.preset.paren"),
            (MultilevelPreset::Bullets, "word.list.preset.bullets"),
            (
                MultilevelPreset::NumberedHeadings,
                "word.list.preset.headings",
            ),
            (MultilevelPreset::Chapters, "word.list.preset.chapters"),
            (MultilevelPreset::Articles, "word.list.preset.articles"),
        ];
        let mut panel = menu_panel("multilevel-menu")
            .w(px(280.0))
            .child(menu_caption(i18n::text("word.list.multilevel_library")))
            .children(
                labels
                    .into_iter()
                    .enumerate()
                    .map(|(index, (preset, label))| {
                        command_row(
                            ("multilevel", index),
                            i18n::text(label),
                            false,
                            ToolCommand::Multilevel(preset),
                            cx,
                        )
                    }),
            );
        if let Some(state) = tools.list.as_ref() {
            panel = panel.child(menu_caption(i18n::text("word.list.change_level")));
            for level in 0..9u8 {
                panel = panel.child(command_row(
                    ("list-level", level as usize),
                    i18n::format(
                        "word.list.level",
                        &[
                            ("n", &(level + 1)),
                            ("sample", &state.definition.preview(level as usize)),
                        ],
                    ),
                    state.level == level,
                    ToolCommand::ListLevel(level),
                    cx,
                ));
            }
        }
        panel.into_any_element()
    });
    let settings = open(ToolMenu::ParagraphSettings).then(|| {
        let state = &tools.paragraph;
        menu_panel("paragraph-settings-menu")
            .w(px(330.0))
            .child(command_row(
                "switch-contextual",
                i18n::text("word.switch.contextual"),
                state.contextual_spacing,
                ToolCommand::Switch(
                    ParagraphSwitch::ContextualSpacing,
                    !state.contextual_spacing,
                ),
                cx,
            ))
            .child(command_row(
                "switch-line-numbers",
                i18n::text("word.switch.line_numbers"),
                state.suppress_line_numbers,
                ToolCommand::Switch(
                    ParagraphSwitch::SuppressLineNumbers,
                    !state.suppress_line_numbers,
                ),
                cx,
            ))
            .child(command_row(
                "switch-hyphenation",
                i18n::text("word.switch.hyphenation"),
                state.dont_hyphenate,
                ToolCommand::Switch(ParagraphSwitch::DontHyphenate, !state.dont_hyphenate),
                cx,
            ))
            .into_any_element()
    });
    let distributed = alignment == Some("distribute");
    group(
        i18n::text("word.group.lists"),
        vec![
            row(vec![
                dropdown(
                    text_button(
                        "bullet-library",
                        "word.list.bullet_library",
                        open(ToolMenu::Bullets),
                        ToolMenu::Bullets,
                        cx,
                    ),
                    bullets,
                ),
                dropdown(
                    text_button(
                        "number-library",
                        "word.list.number_library",
                        open(ToolMenu::Numbering),
                        ToolMenu::Numbering,
                        cx,
                    ),
                    numbering,
                ),
                dropdown(
                    text_button(
                        "multilevel-library",
                        "word.list.multilevel_library",
                        open(ToolMenu::Multilevel),
                        ToolMenu::Multilevel,
                        cx,
                    ),
                    multilevel,
                ),
            ]),
            row(vec![
                div()
                    .id("align-distribute")
                    .flex()
                    .items_center()
                    .h(px(24.0))
                    .px_2()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .when(distributed, |button| button.bg(palette::selected()))
                    .hover(|button| button.bg(palette::hovered()))
                    .on_click(crate::ribbon::listener(Action::Align("distribute"), cx))
                    .child(i18n::text("word.align.distribute"))
                    .into_any_element(),
                dropdown(
                    text_button(
                        "paragraph-settings",
                        "word.paragraph.settings",
                        open(ToolMenu::ParagraphSettings),
                        ToolMenu::ParagraphSettings,
                        cx,
                    ),
                    settings,
                ),
            ]),
        ],
    )
}

/// 挿入 → ドロップキャップ.
pub fn drop_cap_group(ribbon: &Ribbon, tools: &ToolState, cx: &mut Context<DocView>) -> AnyElement {
    let open = ribbon.popup == Some(Popup::Word(ToolMenu::DropCap));
    let current = tools.paragraph.drop_cap;
    let menu = open.then(|| {
        menu_panel("drop-cap-menu")
            .w(px(180.0))
            .child(command_row(
                "drop-cap-none",
                i18n::text("word.drop_cap.none"),
                current.is_none(),
                ToolCommand::DropCap(None),
                cx,
            ))
            .child(command_row(
                "drop-cap-dropped",
                i18n::text("word.drop_cap.dropped"),
                matches!(current, Some((DropCapPosition::Dropped, _))),
                ToolCommand::DropCap(Some((DropCapPosition::Dropped, 3))),
                cx,
            ))
            .child(command_row(
                "drop-cap-margin",
                i18n::text("word.drop_cap.margin"),
                matches!(current, Some((DropCapPosition::InMargin, _))),
                ToolCommand::DropCap(Some((DropCapPosition::InMargin, 3))),
                cx,
            ))
            .into_any_element()
    });
    group(
        i18n::text("word.group.text"),
        vec![row(vec![dropdown(
            text_button(
                "drop-cap",
                "word.drop_cap.title",
                open,
                ToolMenu::DropCap,
                cx,
            ),
            menu,
        )])],
    )
}

fn swatches(colors: &[&str]) -> gpui::Div {
    div()
        .flex()
        .gap_0p5()
        .children(colors.iter().filter_map(|hex| {
            u32::from_str_radix(hex, 16)
                .ok()
                .map(|color| div().size(px(10.0)).bg(rgb(color)))
        }))
}

/// デザイン → ドキュメントの書式設定 and 既定に設定.
pub fn design(ribbon: &Ribbon, tools: &ToolState, cx: &mut Context<DocView>) -> Vec<AnyElement> {
    let open = |menu| ribbon.popup == Some(Popup::Word(menu));
    let state = &tools.design;
    let themes = open(ToolMenu::Themes).then(|| {
        menu_panel("themes-menu")
            .w(px(240.0))
            .children(THEMES.iter().enumerate().map(|(index, theme)| {
                command_row(
                    ("theme", index),
                    theme.name,
                    state.theme.as_deref() == Some(theme.name),
                    ToolCommand::Theme(index),
                    cx,
                )
            }))
            .into_any_element()
    });
    let colors = open(ToolMenu::Colors).then(|| {
        menu_panel("colors-menu")
            .w(px(240.0))
            .children(COLOR_SCHEMES.iter().enumerate().map(|(index, scheme)| {
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(swatches(&scheme.accents))
                    .child(div().flex_1().child(command_row(
                        ("colors", index),
                        scheme.name,
                        state.colors.as_deref() == Some(scheme.name),
                        ToolCommand::Colors(index),
                        cx,
                    )))
            }))
            .into_any_element()
    });
    let fonts = open(ToolMenu::Fonts).then(|| {
        menu_panel("fonts-menu")
            .w(px(280.0))
            .children(FONT_SCHEMES.iter().enumerate().map(|(index, scheme)| {
                command_row(
                    ("fonts", index),
                    format!(
                        "{} — {} / {}",
                        scheme.name, scheme.major_latin, scheme.minor_latin
                    ),
                    state.fonts.as_deref() == Some(scheme.name),
                    ToolCommand::Fonts(index),
                    cx,
                )
            }))
            .into_any_element()
    });
    let effects = open(ToolMenu::Effects).then(|| {
        menu_panel("effects-menu")
            .w(px(200.0))
            .children(EFFECT_SCHEMES.iter().enumerate().map(|(index, scheme)| {
                command_row(
                    ("effects", index),
                    scheme.name,
                    state.effects.as_deref() == Some(scheme.name),
                    ToolCommand::Effects(index),
                    cx,
                )
            }))
            .into_any_element()
    });
    let english = i18n::is_english();
    let sets = open(ToolMenu::StyleSets).then(|| {
        menu_panel("style-sets-menu")
            .w(px(220.0))
            .children(style_sets().iter().enumerate().map(|(index, set)| {
                command_row(
                    ("style-set", index),
                    if english { set.name } else { set.label_ja },
                    false,
                    ToolCommand::StyleSet(index),
                    cx,
                )
            }))
            .into_any_element()
    });
    let spacing = open(ToolMenu::Spacing).then(|| {
        menu_panel("spacing-sets-menu")
            .w(px(200.0))
            .children(PARAGRAPH_SPACINGS.iter().enumerate().map(
                |(index, (key, label, _, after, line))| {
                    command_row(
                        ("paragraph-spacing", index),
                        format!(
                            "{} ({after} pt, {line}×)",
                            if english { *key } else { *label }
                        ),
                        false,
                        ToolCommand::Spacing(index),
                        cx,
                    )
                },
            ))
            .into_any_element()
    });
    let button =
        |id: &'static str, label: &'static str, menu: ToolMenu, cx: &mut Context<DocView>| {
            text_button(id, label, ribbon.popup == Some(Popup::Word(menu)), menu, cx)
        };
    vec![
        group(
            i18n::text("word.group.document_format"),
            vec![
                row(vec![
                    dropdown(
                        button("themes", "word.design.themes", ToolMenu::Themes, cx),
                        themes,
                    ),
                    dropdown(
                        button(
                            "style-sets",
                            "word.design.style_sets",
                            ToolMenu::StyleSets,
                            cx,
                        ),
                        sets,
                    ),
                    dropdown(
                        button("colors", "word.design.colors", ToolMenu::Colors, cx),
                        colors,
                    ),
                ]),
                row(vec![
                    dropdown(
                        button("fonts", "word.design.fonts", ToolMenu::Fonts, cx),
                        fonts,
                    ),
                    dropdown(
                        button("effects", "word.design.effects", ToolMenu::Effects, cx),
                        effects,
                    ),
                    dropdown(
                        button("spacing", "word.design.spacing", ToolMenu::Spacing, cx),
                        spacing,
                    ),
                ]),
            ],
        ),
        group(
            i18n::text("word.group.defaults"),
            vec![row(vec![div()
                .id("set-as-default")
                .flex()
                .items_center()
                .h(px(24.0))
                .px_2()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .hover(|button| button.bg(palette::hovered()))
                .on_click(crate::ribbon::listener(
                    Action::Word(ToolCommand::SetAsDefault),
                    cx,
                ))
                .child(i18n::text("word.design.set_default"))
                .into_any_element()])],
        ),
    ]
}

/// Tab and ⇧Tab at a list item's start (or over several items) move the
/// items a level, as in Word; `false` where Tab is a tab.
pub fn tab_in_list(document: &mut EditableDocument, backwards: bool) -> Option<Result<(), String>> {
    let at_start = match document.selection() {
        Some((anchor, focus)) => anchor.0 != focus.0 || anchor.1.min(focus.1) == 0,
        None => document.caret().is_some_and(|(_, offset)| offset == 0),
    };
    if !at_start {
        return None;
    }
    let state = document.list_state()?;
    let (paragraph, _) = document.caret()?;
    let first_item = !(0..paragraph).any(|index| {
        document
            .word_document()
            .paragraph(index)
            .and_then(|paragraph| paragraph.properties.as_ref())
            .and_then(|props| props.numbering.as_ref())
            .and_then(|numbering| numbering.id.as_deref())
            == Some(state.num_id.as_str())
    });
    if first_item && document.selection().is_none() {
        return None;
    }
    Some(document.change_list_level(if backwards { -1 } else { 1 }))
}

//! The ホーム tab's font group, beyond its first two rows: 文字種の変換,
//! 下線のスタイル and 下線の色, 文字の網かけ, 囲み線, 拡張書式 and the
//! フォント dialog's launcher; and the lists the first two rows drop — the
//! installed fonts and the font colour's テーマの色.

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_office_model::word::font::{
    theme_palette, AsianLayout, HeadlessChangeCaseMode, SCALES, STANDARD_COLORS, UNDERLINE_STYLES,
};
use jade_office_model::word::FormatState;

use super::{installed_font_lists, CharAction, CharPopup};
use crate::ribbon::{
    dropdown, listener, menu_caption, menu_panel, menu_row, row, Action, Popup, Ribbon,
};
use crate::tooltip::Tooltip;
use crate::{i18n, theme, DocumentView};

fn action(action: CharAction) -> Action {
    Action::Characters(action)
}

/// A text glyph button; `active` shows it pressed.
fn glyph_button(
    id: &'static str,
    glyph: impl Into<SharedString>,
    tooltip: &'static str,
    active: bool,
    on_click: Action,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(px(26.0))
        .h(px(26.0))
        .px_1()
        .rounded_md()
        .text_sm()
        .text_color(theme::text())
        .when(active, |button| button.bg(theme::selected()))
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(on_click, cx))
        .child(glyph.into())
        .into_any_element()
}

/// A glyph with a chevron, which opens one of this row's lists.
fn glyph_menu(
    id: &'static str,
    glyph: impl Into<SharedString>,
    tooltip: &'static str,
    open: bool,
    popup: CharPopup,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_0p5()
        .h(px(26.0))
        .px_1()
        .rounded_md()
        .text_sm()
        .text_color(theme::text())
        .cursor_pointer()
        .when(open, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(Action::Popup(Popup::Characters(popup)), cx))
        .child(glyph.into())
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element()
}

fn case_label(mode: HeadlessChangeCaseMode) -> &'static str {
    i18n::text(match mode {
        HeadlessChangeCaseMode::Sentence => "chars.case.sentence",
        HeadlessChangeCaseMode::Lower => "chars.case.lower",
        HeadlessChangeCaseMode::Upper => "chars.case.upper",
        HeadlessChangeCaseMode::Title => "chars.case.title",
        HeadlessChangeCaseMode::Toggle => "chars.case.toggle",
        HeadlessChangeCaseMode::HalfWidth => "chars.case.half_width",
        HeadlessChangeCaseMode::FullWidth => "chars.case.full_width",
        HeadlessChangeCaseMode::Katakana => "chars.case.katakana",
        HeadlessChangeCaseMode::Hiragana => "chars.case.hiragana",
    })
}

/// The font group's third row.
pub fn font_row(
    ribbon: &Ribbon,
    state: &FormatState,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let open = ribbon.popup;
    let is_open = |popup: CharPopup| open == Some(Popup::Characters(popup));
    let character = &state.character;
    let case_menu = is_open(CharPopup::ChangeCase).then(|| {
        let modes = [
            HeadlessChangeCaseMode::Sentence,
            HeadlessChangeCaseMode::Lower,
            HeadlessChangeCaseMode::Upper,
            HeadlessChangeCaseMode::Title,
            HeadlessChangeCaseMode::Toggle,
            HeadlessChangeCaseMode::HalfWidth,
            HeadlessChangeCaseMode::FullWidth,
            HeadlessChangeCaseMode::Katakana,
            HeadlessChangeCaseMode::Hiragana,
        ];
        menu_panel("case-menu")
            .w(px(240.0))
            .children(modes.into_iter().enumerate().map(|(index, mode)| {
                menu_row(
                    ("case", index),
                    case_label(mode),
                    false,
                    action(CharAction::ChangeCase(mode)),
                    cx,
                )
            }))
            .into_any_element()
    });
    let underline = character.underline.clone().unwrap_or_else(|| "none".into());
    let underline_menu = is_open(CharPopup::Underline).then(|| {
        menu_panel("underline-menu")
            .w(px(220.0))
            .max_h(px(420.0))
            .overflow_y_scroll()
            .children(UNDERLINE_STYLES.iter().enumerate().map(|(index, kind)| {
                menu_row(
                    ("underline-kind", index),
                    super::font::underline_label(kind),
                    underline == *kind,
                    action(CharAction::Underline(kind)),
                    cx,
                )
            }))
            .child(menu_caption(i18n::text("chars.font.underline_color")))
            .child(menu_row(
                "underline-color-auto",
                i18n::text("doc.color.auto"),
                false,
                action(CharAction::UnderlineColor("auto".into())),
                cx,
            ))
            .child(div().flex().flex_wrap().gap_1().p_1().children(
                STANDARD_COLORS.iter().enumerate().map(|(index, color)| {
                    color_square(
                        ("underline-color", index),
                        color,
                        action(CharAction::UnderlineColor(color.to_string())),
                        cx,
                    )
                }),
            ))
            .into_any_element()
    });
    let layout_menu = is_open(CharPopup::AsianLayout).then(|| {
        let entries = [
            (
                "chars.asian.vertical",
                AsianLayout::Vertical { compress: false },
            ),
            (
                "chars.asian.vertical_fit",
                AsianLayout::Vertical { compress: true },
            ),
            ("chars.asian.combine", AsianLayout::Combine),
            (
                "chars.asian.warichu",
                AsianLayout::WarichuBrackets("round".into()),
            ),
            (
                "chars.asian.warichu_none",
                AsianLayout::WarichuBrackets("none".into()),
            ),
            ("chars.asian.remove", AsianLayout::Remove),
        ];
        menu_panel("asian-menu")
            .w(px(220.0))
            .children(
                entries
                    .into_iter()
                    .enumerate()
                    .map(|(index, (label, layout))| {
                        menu_row(
                            ("asian", index),
                            i18n::text(label),
                            false,
                            action(CharAction::AsianLayout(layout)),
                            cx,
                        )
                    }),
            )
            .child(menu_caption(i18n::text("chars.asian.scale")))
            .children(SCALES.iter().enumerate().map(|(index, percent)| {
                menu_row(
                    ("asian-scale", index),
                    format!("{percent}%"),
                    character.scale.unwrap_or(100.0) == *percent,
                    action(CharAction::Scale(*percent)),
                    cx,
                )
            }))
            .into_any_element()
    });
    let shaded = character
        .shading
        .as_deref()
        .is_some_and(|fill| fill != "none");
    let boxed = character.border == Some(true);
    row(vec![
        dropdown(
            glyph_menu(
                "change-case",
                "Aa",
                i18n::text("chars.case.title_tip"),
                is_open(CharPopup::ChangeCase),
                CharPopup::ChangeCase,
                cx,
            ),
            case_menu,
        ),
        dropdown(
            glyph_menu(
                "underline-style",
                "U̲",
                i18n::text("chars.font.underline"),
                is_open(CharPopup::Underline),
                CharPopup::Underline,
                cx,
            ),
            underline_menu,
        ),
        glyph_button(
            "char-shading",
            "Ａ",
            i18n::text("chars.shading"),
            shaded,
            action(CharAction::Shading),
            cx,
        ),
        glyph_button(
            "char-border",
            "[Ａ]",
            i18n::text("chars.border"),
            boxed,
            action(CharAction::Border),
            cx,
        ),
        dropdown(
            glyph_menu(
                "asian-layout",
                "拡",
                i18n::text("chars.asian.title"),
                is_open(CharPopup::AsianLayout),
                CharPopup::AsianLayout,
                cx,
            ),
            layout_menu,
        ),
        glyph_button(
            "ruby-dialog",
            "亜",
            i18n::text("chars.ruby.title"),
            false,
            action(CharAction::OpenRuby),
            cx,
        ),
        glyph_button(
            "enclose-dialog",
            "㊀",
            i18n::text("chars.enclose.title"),
            false,
            action(CharAction::OpenEnclose),
            cx,
        ),
        glyph_button(
            "typography-dialog",
            "体裁",
            i18n::text("chars.typography.title"),
            false,
            action(CharAction::OpenTypography),
            cx,
        ),
        glyph_button(
            "font-dialog",
            "…",
            i18n::text("chars.font.launcher"),
            false,
            action(CharAction::OpenFontDialog),
            cx,
        ),
    ])
}

fn color_square(
    id: impl Into<gpui::ElementId>,
    color: &str,
    on_click: Action,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let rgb = u32::from_str_radix(color, 16).unwrap_or(0);
    div()
        .id(id)
        .size(px(18.0))
        .rounded_sm()
        .bg(gpui::rgb(rgb))
        .border_1()
        .border_color(theme::border())
        .cursor_pointer()
        .hover(|square| square.border_color(theme::accent()))
        .tooltip(Tooltip::text(format!("#{color}")))
        .when(crate::word_views::mini::previewable(&on_click), |square| {
            square.on_hover(crate::word_views::mini::hover(on_click.clone(), cx))
        })
        .on_click(listener(on_click, cx))
        .into_any_element()
}

/// The font family list: the installed fonts, Japanese ones first.
pub fn font_family_menu(family: &str, cx: &mut Context<DocumentView>) -> AnyElement {
    let (japanese, latin) = installed_font_lists(cx);
    let mut panel = menu_panel("family-menu")
        .w(px(240.0))
        .max_h(px(420.0))
        .overflow_y_scroll()
        .child(menu_caption(i18n::text("chars.font.japanese")));
    for (index, name) in japanese.iter().enumerate() {
        panel = panel.child(menu_row(
            ("family-ja", index),
            name.clone(),
            family == name,
            Action::FontFamilyName(name.clone()),
            cx,
        ));
    }
    panel = panel.child(menu_caption(i18n::text("chars.font.all_fonts")));
    for (index, name) in latin.iter().enumerate() {
        panel = panel.child(menu_row(
            ("family-all", index),
            name.clone(),
            family == name,
            Action::FontFamilyName(name.clone()),
            cx,
        ));
    }
    panel.into_any_element()
}

/// The font colour list: 自動, テーマの色, 標準の色 and その他の色.
pub fn font_color_menu(
    theme_colors: &[(String, String)],
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let palette = theme_palette(theme_colors);
    let mut panel = menu_panel("color-menu").w(px(250.0)).child(menu_row(
        "color-auto",
        i18n::text("doc.color.auto"),
        false,
        Action::FontColor("auto".to_string()),
        cx,
    ));
    panel = panel.child(menu_caption(i18n::text("chars.color.theme")));
    for (row_index, swatches) in palette.iter().enumerate() {
        panel = panel.child(
            div()
                .flex()
                .gap_1()
                .px_1()
                .when(row_index == 0, |row| row.mb_1())
                .children(swatches.iter().enumerate().map(|(column, swatch)| {
                    color_square(
                        ("theme-color", row_index * 10 + column),
                        &swatch.color,
                        action(CharAction::ThemeColor(row_index, column)),
                        cx,
                    )
                })),
        );
    }
    panel
        .child(menu_caption(i18n::text("chars.color.standard")))
        .child(
            div()
                .flex()
                .gap_1()
                .px_1()
                .children(STANDARD_COLORS.iter().enumerate().map(|(index, color)| {
                    color_square(
                        ("standard-color", index),
                        color,
                        Action::FontColor(color.to_string()),
                        cx,
                    )
                })),
        )
        .child(menu_row(
            "color-more",
            i18n::text("chars.color.more"),
            false,
            action(CharAction::MoreFontColors),
            cx,
        ))
        .into_any_element()
}

/// The ▾ beside 貼り付け: 貼り付けのオプション and 形式を選択して貼り付け.
pub fn paste_options(ribbon: &Ribbon, cx: &mut Context<DocumentView>) -> AnyElement {
    use jade_office_model::word::clipboard::PasteMode;
    let open = ribbon.popup == Some(Popup::Characters(CharPopup::PasteOptions));
    let menu = open.then(|| {
        menu_panel("paste-menu")
            .w(px(240.0))
            .children(PasteMode::ALL.into_iter().enumerate().map(|(index, mode)| {
                menu_row(
                    ("paste-mode", index),
                    super::paste::mode_label(mode),
                    false,
                    action(CharAction::Paste(mode)),
                    cx,
                )
            }))
            .child(menu_row(
                "paste-special",
                i18n::text("chars.paste.title_menu"),
                false,
                action(CharAction::OpenPasteSpecial),
                cx,
            ))
            .into_any_element()
    });
    dropdown(
        glyph_menu(
            "paste-options",
            "",
            i18n::text("chars.paste.options"),
            open,
            CharPopup::PasteOptions,
            cx,
        ),
        menu,
    )
}

/// 挿入's 記号と特殊文字 and 日付と時刻.
pub fn insert_group(cx: &mut Context<DocumentView>) -> AnyElement {
    let button = |id: &'static str,
                  label: &'static str,
                  on_click: CharAction,
                  cx: &mut Context<DocumentView>| {
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
            .hover(|button| button.bg(theme::hovered()))
            .on_click(listener(action(on_click), cx))
            .child(i18n::text(label))
            .into_any_element()
    };
    crate::ribbon::group(
        i18n::text("chars.insert.group"),
        vec![row(vec![
            button(
                "insert-symbol",
                "chars.symbol.title_menu",
                CharAction::OpenSymbols,
                cx,
            ),
            button(
                "insert-date",
                "chars.date.title_menu",
                CharAction::OpenDateTime,
                cx,
            ),
        ])],
    )
}

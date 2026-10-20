//! The ribbon's page tools: レイアウト's ページ設定 — 余白, 印刷の向き, サイズ,
//! 段組み, 区切り — and 挿入's ヘッダーとフッター — the header and footer
//! text, typed into a bar over the pages, and ページ番号. The native app has
//! the same (`jade-gpui-app/src/word_page.rs`), over the same commands
//! (`jade-office-model`'s `WordPage`).

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_web::word::page::{
    MarginPreset, NumberAlignment, PageCommand, PageSetup, Paper, SectionBreak, Story,
};

use crate::ribbon::{
    dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup, Ribbon,
};
use crate::tooltip::Tooltip;
use crate::view::DocView;
use crate::{i18n, palette};

/// A text button with a chevron that opens a list.
fn list_button(
    id: &'static str,
    label: &'static str,
    open: bool,
    popup: Popup,
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
        .on_click(listener(Action::Popup(popup), cx))
        .child(label)
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(palette::dim()),
        )
        .into_any_element()
}

/// A text button that runs `action`; `active` shows it pressed.
fn text_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    active: bool,
    action: Action,
    cx: &mut Context<DocView>,
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
        .text_color(palette::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(active, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(action, cx))
        .child(label)
        .into_any_element()
}

fn margin_label(preset: MarginPreset) -> &'static str {
    i18n::text(match preset {
        MarginPreset::Normal => "page.margins.normal",
        MarginPreset::Narrow => "page.margins.narrow",
        MarginPreset::Moderate => "page.margins.moderate",
        MarginPreset::Wide => "page.margins.wide",
    })
}

fn break_label(kind: SectionBreak) -> &'static str {
    i18n::text(match kind {
        SectionBreak::NextPage => "page.break.next_page",
        SectionBreak::Continuous => "page.break.continuous",
        SectionBreak::EvenPage => "page.break.even_page",
        SectionBreak::OddPage => "page.break.odd_page",
    })
}

/// Millimetres, as the 余白 list describes each preset.
fn millimetres(points: f32) -> String {
    format!("{:.1}", points * 25.4 / 72.0)
}

/// レイアウト → ページ設定.
pub fn page_setup(
    ribbon: &Ribbon,
    setup: &PageSetup,
    preset: Option<MarginPreset>,
    japanese: bool,
    cx: &mut Context<DocView>,
) -> AnyElement {
    let open = ribbon.popup;
    let page = |command| Action::Page(command);
    let margins_menu = (open == Some(Popup::Margins)).then(|| {
        menu_panel("margins-menu")
            .w(px(250.0))
            .children(
                MarginPreset::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        let margins = entry.margins(japanese);
                        menu_row(
                            ("margins", index),
                            format!(
                                "{}  {} {} · {} {} mm",
                                margin_label(entry),
                                i18n::text("page.margins.top_bottom"),
                                millimetres(margins.top),
                                i18n::text("page.margins.left_right"),
                                millimetres(margins.left),
                            ),
                            preset == Some(entry),
                            page(PageCommand::Margins(entry)),
                            cx,
                        )
                    }),
            )
            .into_any_element()
    });
    let paper = Paper::of(setup.width, setup.height);
    let size_menu = (open == Some(Popup::Paper)).then(|| {
        menu_panel("paper-menu")
            .w(px(200.0))
            .children(Paper::ALL.into_iter().enumerate().map(|(index, entry)| {
                let (width, height) = entry.size();
                menu_row(
                    ("paper", index),
                    format!(
                        "{}  {} × {} mm",
                        entry.name(),
                        millimetres(width).trim_end_matches(".0"),
                        millimetres(height).trim_end_matches(".0"),
                    ),
                    paper == Some(entry),
                    page(PageCommand::Paper(entry)),
                    cx,
                )
            }))
            .into_any_element()
    });
    let columns_menu = (open == Some(Popup::Columns)).then(|| {
        menu_panel("columns-menu")
            .w(px(120.0))
            .children((1..=3).map(|count| {
                menu_row(
                    ("columns", count as usize),
                    i18n::format("page.columns.count", &[("n", &count)]),
                    setup.columns == count,
                    page(PageCommand::Columns(count)),
                    cx,
                )
            }))
            .into_any_element()
    });
    let breaks_menu = (open == Some(Popup::Breaks)).then(|| {
        let kinds = [
            SectionBreak::NextPage,
            SectionBreak::Continuous,
            SectionBreak::EvenPage,
            SectionBreak::OddPage,
        ];
        menu_panel("breaks-menu")
            .w(px(200.0))
            .child(menu_caption(i18n::text("page.break.sections")))
            .children(kinds.into_iter().enumerate().map(|(index, kind)| {
                menu_row(
                    ("break", index),
                    break_label(kind),
                    false,
                    page(PageCommand::Break(kind)),
                    cx,
                )
            }))
            .into_any_element()
    });
    group(
        i18n::text("page.group.setup"),
        vec![
            row(vec![
                dropdown(
                    list_button(
                        "margins",
                        i18n::text("page.margins"),
                        open == Some(Popup::Margins),
                        Popup::Margins,
                        cx,
                    ),
                    margins_menu,
                ),
                text_button(
                    "portrait",
                    i18n::text("page.portrait"),
                    i18n::text("page.orientation"),
                    !setup.landscape,
                    page(PageCommand::Landscape(false)),
                    cx,
                ),
                text_button(
                    "landscape",
                    i18n::text("page.landscape"),
                    i18n::text("page.orientation"),
                    setup.landscape,
                    page(PageCommand::Landscape(true)),
                    cx,
                ),
            ]),
            row(vec![
                dropdown(
                    list_button(
                        "paper",
                        i18n::text("page.size"),
                        open == Some(Popup::Paper),
                        Popup::Paper,
                        cx,
                    ),
                    size_menu,
                ),
                dropdown(
                    list_button(
                        "columns",
                        i18n::text("page.columns"),
                        open == Some(Popup::Columns),
                        Popup::Columns,
                        cx,
                    ),
                    columns_menu,
                ),
                dropdown(
                    list_button(
                        "breaks",
                        i18n::text("page.breaks"),
                        open == Some(Popup::Breaks),
                        Popup::Breaks,
                        cx,
                    ),
                    breaks_menu,
                ),
            ]),
        ],
    )
}

/// 挿入 → ヘッダーとフッター.
pub fn header_footer(ribbon: &Ribbon, cx: &mut Context<DocView>) -> AnyElement {
    let open = ribbon.popup;
    let numbers_menu =
        (open == Some(Popup::PageNumbers)).then(|| {
            let spots = [
                (Story::Header, NumberAlignment::Left, "page.number.top_left"),
                (
                    Story::Header,
                    NumberAlignment::Center,
                    "page.number.top_center",
                ),
                (
                    Story::Header,
                    NumberAlignment::Right,
                    "page.number.top_right",
                ),
                (
                    Story::Footer,
                    NumberAlignment::Left,
                    "page.number.bottom_left",
                ),
                (
                    Story::Footer,
                    NumberAlignment::Center,
                    "page.number.bottom_center",
                ),
                (
                    Story::Footer,
                    NumberAlignment::Right,
                    "page.number.bottom_right",
                ),
            ];
            menu_panel("page-number-menu")
                .w(px(200.0))
                .children(spots.into_iter().enumerate().map(
                    |(index, (story, alignment, label))| {
                        menu_row(
                            ("page-number", index),
                            i18n::text(label),
                            false,
                            Action::Page(PageCommand::PageNumber(story, alignment)),
                            cx,
                        )
                    },
                ))
                .child(menu_row(
                    "page-number-remove",
                    i18n::text("page.number.remove"),
                    false,
                    Action::Page(PageCommand::RemovePageNumbers),
                    cx,
                ))
                .into_any_element()
        });
    group(
        i18n::text("page.group.header_footer"),
        vec![row(vec![
            text_button(
                "edit-header",
                i18n::text("page.header"),
                i18n::text("page.header_title"),
                false,
                Action::EditStory(Story::Header),
                cx,
            ),
            text_button(
                "edit-footer",
                i18n::text("page.footer"),
                i18n::text("page.footer_title"),
                false,
                Action::EditStory(Story::Footer),
                cx,
            ),
            dropdown(
                list_button(
                    "page-numbers",
                    i18n::text("page.number"),
                    open == Some(Popup::PageNumbers),
                    Popup::PageNumbers,
                    cx,
                ),
                numbers_menu,
            ),
        ])],
    )
}

/// The text of a header or footer being edited, in a bar over the pages.
pub struct StoryBar {
    pub story: Story,
    pub text: jade_web::text::TextEdit,
}

impl StoryBar {
    pub fn label(&self) -> SharedString {
        i18n::text(match self.story {
            Story::Header => "page.header_editing",
            Story::Footer => "page.footer_editing",
        })
        .into()
    }
}

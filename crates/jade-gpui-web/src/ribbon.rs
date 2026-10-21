//! The ribbon over a Word document's pages: jade-ide's style menu and table
//! tools (`JadeEditorStyleMenu`, `JadeAdvancedRibbon`) rebuilt in gpui.
//!
//! - ホーム: the font (family, size, grow and shrink, bold … superscript,
//!   colour, highlight, clear), the paragraph (lists, indent, alignment, line
//!   spacing, shading, borders), the style gallery, undo and redo.
//! - 挿入: a table, picked on a 10 × 8 grid.
//! - レイアウト: the table tools — rows, columns, the table, merging, and the
//!   table-style and cell-format lists — live while the caret is in a table.
//!
//! The lists drop under their buttons over everything else, with a backdrop
//! that closes them on a click anywhere outside. What the buttons show as
//! current is read from the document at the caret ([`FormatState`]); what
//! they do is [`Action`], which the view runs against `jade-web`'s document.

use gpui::{
    anchored, deferred, div, point, prelude::*, px, rgb, svg, AnyElement, ClickEvent, Context,
    ElementId, MouseButton, Pixels, SharedString, Size, Window,
};
use jade_web::editing::{
    style_gallery, CharacterFormat, EditableDocument, FormatState, GalleryStyle, ListKind,
    ParagraphBorders, TableBorders, TableCommand, WordFormatting,
};

use jade_web::word::page::{MarginPreset, PageCommand, PageSetup, Story};

use crate::tooltip::Tooltip;
use crate::view::DocView;
use crate::{i18n, palette};

/// jade-ide's font list (`FONT_FAMILIES`).
const FONT_FAMILIES: [&str; 10] = [
    "ＭＳ 明朝",
    "游明朝",
    "Hiragino Mincho ProN",
    "Hiragino Sans",
    "Noto Sans JP",
    "Yu Mincho",
    "Noto Serif JP",
    "MS Mincho",
    "Times New Roman",
    "Arial",
];
/// Word's ribbon sizes (`WORD_FONT_SIZES`).
const FONT_SIZES: [f32; 17] = [
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0,
    72.0,
];
/// What an empty size or family box falls back to, as in jade-ide.
const DEFAULT_FAMILY: &str = "游明朝";
const DEFAULT_SIZE: f32 = 12.0;
/// The font colour swatches, jade-ide's `textColors`.
const TEXT_COLORS: [u32; 22] = [
    0x000000, 0x1a1a2e, 0x434343, 0x666666, 0x999999, 0xb7b7b7, 0xdc2626, 0xea580c, 0xca8a04,
    0x16a34a, 0x0d9488, 0x2563eb, 0x7c3aed, 0xdb2777, 0xfca5a5, 0xfdba74, 0xfde047, 0x86efac,
    0x5eead4, 0x93c5fd, 0xc4b5fd, 0xf9a8d4,
];
/// Word's highlight names and what they look like.
const HIGHLIGHTS: [(&str, u32); 15] = [
    ("yellow", 0xffff00),
    ("green", 0x00ff00),
    ("cyan", 0x00ffff),
    ("magenta", 0xff00ff),
    ("blue", 0x0000ff),
    ("red", 0xff0000),
    ("darkBlue", 0x000080),
    ("darkCyan", 0x008080),
    ("darkGreen", 0x008000),
    ("darkMagenta", 0x800080),
    ("darkRed", 0x800000),
    ("darkYellow", 0x808000),
    ("darkGray", 0x808080),
    ("lightGray", 0xc0c0c0),
    ("black", 0x000000),
];
const LINE_SPACINGS: [(&str, f32); 4] = [("1.0", 1.0), ("1.15", 1.15), ("1.5", 1.5), ("2.0", 2.0)];
/// The paragraph dialog's shading swatches.
const SHADINGS: [u32; 4] = [0xfff2cc, 0xe2f0d9, 0xddebf7, 0xededed];
/// The cell-format list's shading, its default first.
const CELL_SHADINGS: [u32; 5] = [0xd9eaf7, 0xfff2cc, 0xe2f0d9, 0xededed, 0xfce4d6];
/// The engine's table presets, with the header and band colours they lay
/// down, for the swatch beside each name.
const TABLE_PRESETS: [(&str, &str, u32, u32); 5] = [
    ("plain", "ribbon.preset.plain", 0xffffff, 0xffffff),
    ("grid", "ribbon.preset.grid", 0xffffff, 0xffffff),
    ("lightBlue", "ribbon.preset.light_blue", 0xd9eaf7, 0xffffff),
    ("blueBands", "ribbon.preset.blue_bands", 0xbdd7ee, 0xddebf7),
    ("grayBands", "ribbon.preset.gray_bands", 0xd9e1f2, 0xe7e6e6),
];
/// The table picker's grid, jade-ide's 10 columns by 8 rows.
const GRID_COLUMNS: usize = 10;
const GRID_ROWS: usize = 8;
/// How many gallery entries sit in the ribbon; the rest are in its list.
const GALLERY_INLINE: usize = 6;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Home,
    Insert,
    Layout,
    Review,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Home, Tab::Insert, Tab::Layout, Tab::Review];

    fn label(self) -> &'static str {
        i18n::text(match self {
            Tab::Home => "doc.tab.home",
            Tab::Insert => "doc.tab.insert",
            Tab::Layout => "doc.tab.layout",
            Tab::Review => "doc.tab.review",
        })
    }
}

/// The lists the ribbon drops under its buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Popup {
    FontFamily,
    FontSize,
    FontColor,
    Highlight,
    LineSpacing,
    Shading,
    Borders,
    Styles,
    InsertTable,
    TableStyle,
    CellFormat,
    /// レイアウト's ページ設定 and 挿入's ページ番号 (`word_page`).
    Margins,
    Paper,
    Columns,
    Breaks,
    PageNumbers,
    /// レイアウト's 行番号, 文字列の方向 and ページの色, 挿入's 表紙
    /// (`view::word_setup`).
    LineNumbers,
    TextDirection,
    PageColor,
    Cover,
}

/// The ribbon's own state, kept with the document it formats.
#[derive(Default)]
pub struct Ribbon {
    pub tab: Tab,
    pub popup: Option<Popup>,
    /// The table picker's highlighted size, columns by rows, 1-based.
    pub grid: (usize, usize),
    gallery: Vec<GalleryStyle>,
    /// The document's default paragraph style, which 標準 stands for.
    default_style: Option<String>,
}

impl Ribbon {
    /// Reads the document's paragraph styles into the gallery. Again after a
    /// style is applied: a missing built-in is added on first use.
    pub fn read_styles(&mut self, document: &EditableDocument) {
        let styles = document.paragraph_styles();
        self.default_style = styles
            .iter()
            .find(|style| style.is_default)
            .map(|style| style.id.clone());
        self.gallery = style_gallery(&styles);
    }

    /// Opens a list, or closes it when it is the one open.
    pub fn toggle(&mut self, popup: Popup) {
        self.popup = (self.popup != Some(popup)).then_some(popup);
        self.grid = (1, 1);
    }
}

/// What a ribbon control does.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Tab(Tab),
    Popup(Popup),
    ClosePopup,
    /// The table picker's pointer moved onto a size.
    Grid(usize, usize),
    Undo,
    Redo,
    Toggle(CharacterFormat),
    ClearFormatting,
    FontFamily(&'static str),
    FontSize(f32),
    StepFontSize(bool),
    /// `RRGGBB`, or `auto`.
    FontColor(String),
    Highlight(&'static str),
    List(bool),
    Indent(bool),
    Align(&'static str),
    LineSpacing(f32),
    Shading(Option<String>),
    ParagraphBorders(ParagraphBorders),
    Style(Option<String>),
    InsertTable(usize, usize),
    Table(TableCommand),
    /// ページ設定 and ページ番号.
    Page(PageCommand),
    /// ヘッダーの編集 / フッターの編集: the bar over the pages.
    EditStory(Story),
    /// The 校閲 tab.
    Review(crate::word_review::ReviewAction),
    /// 挿入 → 画像: a picture picked from the device.
    InsertPicture,
    /// 文字列の折り返し for the picture selected.
    Wrap(jade_web::word::picture::Wrap),
    /// The ページ設定 dialogs and menus (`view::word_setup`).
    Setup(crate::view::word_setup::SetupAction),
}

impl Action {
    /// Whether the action only opens, closes or moves within the ribbon.
    pub fn is_chrome(&self) -> bool {
        matches!(
            self,
            Action::Tab(_) | Action::Popup(_) | Action::ClosePopup | Action::Grid(..)
        )
    }
}

fn hex(color: u32) -> String {
    format!("{color:06X}")
}

pub(crate) fn listener(
    action: Action,
    cx: &mut Context<DocView>,
) -> impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static {
    // `cx.listener`, without its borrow of `cx` in the returned type.
    let view = cx.weak_entity();
    move |_event, window, cx| {
        view.update(cx, |view, cx| {
            view.ribbon_action(action.clone(), window, cx)
        })
        .ok();
    }
}

/// A square icon button with a tooltip; `active` shows it pressed.
fn icon_button(
    id: impl Into<ElementId>,
    icon: &'static str,
    tooltip: &'static str,
    active: bool,
    enabled: bool,
    action: Action,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(26.0))
        .rounded_md()
        .when(active, |button| button.bg(palette::selected()))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(palette::hovered()))
                .on_click(listener(action, cx))
        })
        .tooltip(Tooltip::text(tooltip))
        .child(svg().path(icon).size_4().text_color(if !enabled {
            palette::faint()
        } else if active {
            palette::accent()
        } else {
            palette::text()
        }))
        .into_any_element()
}

/// A button with an icon and a label, for the table tools, whose icons
/// alone would not say which way a row goes.
fn labeled_button(
    id: impl Into<ElementId>,
    icon: &'static str,
    label: &'static str,
    enabled: bool,
    action: Action,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_1p5()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .text_color(if enabled {
            palette::text()
        } else {
            palette::faint()
        })
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(palette::hovered()))
                .on_click(listener(action, cx))
        })
        .when(!enabled, |button| {
            button.tooltip(Tooltip::text(i18n::text("ribbon.needs_table")))
        })
        .child(svg().path(icon).size_3p5().text_color(if enabled {
            palette::dim()
        } else {
            palette::faint()
        }))
        .child(label)
        .into_any_element()
}

/// An icon with a chevron, which opens a list.
fn menu_button(
    id: impl Into<ElementId>,
    icon: &'static str,
    tooltip: &'static str,
    swatch: Option<u32>,
    open: bool,
    popup: Popup,
    cx: &mut Context<DocView>,
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
        .cursor_pointer()
        .when(open, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(Action::Popup(popup), cx))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .child(svg().path(icon).size_4().text_color(palette::text()))
                .children(
                    swatch.map(|color| div().mt(px(-1.0)).w(px(14.0)).h(px(3.0)).bg(rgb(color))),
                ),
        )
        .child(
            svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(palette::dim()),
        )
        .into_any_element()
}

/// A box showing a value, which opens its list: the family and size.
fn combo_box(
    id: impl Into<ElementId>,
    value: String,
    width: f32,
    tooltip: &'static str,
    open: bool,
    popup: Popup,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .w(px(width))
        .h(px(24.0))
        .pl_2()
        .pr_1()
        .rounded_md()
        .border_1()
        .border_color(if open {
            palette::accent()
        } else {
            palette::border()
        })
        .cursor_pointer()
        .hover(|button| button.bg(palette::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(Action::Popup(popup), cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_xs()
                .text_color(palette::text())
                .child(value),
        )
        .child(
            svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(palette::dim()),
        )
        .into_any_element()
}

/// A trigger with its list under it while open.
pub(crate) fn dropdown(trigger: AnyElement, menu: Option<AnyElement>) -> AnyElement {
    div()
        .relative()
        .flex_none()
        .child(trigger)
        .children(menu.map(|menu| {
            div().absolute().top_full().left_0().child(
                deferred(anchored().snap_to_window_with_margin(px(8.0)).child(menu))
                    .with_priority(2),
            )
        }))
        .into_any_element()
}

/// The panel a list is drawn on.
pub(crate) fn menu_panel(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .occlude()
        .mt_1()
        .p_1()
        .flex()
        .flex_col()
        .rounded_md()
        .bg(palette::panel())
        .border_1()
        .border_color(palette::border())
        .shadow_lg()
        .text_xs()
        .text_color(palette::text())
}

/// One row of a list: a check for the current value, then the label.
pub(crate) fn menu_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
    action: Action,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .h(px(26.0))
        .pl_1()
        .pr_3()
        .rounded_md()
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|row| row.bg(palette::hovered()))
        .on_click(listener(action, cx))
        .child(div().size_4().flex_none().when(checked, |cell| {
            cell.child(
                svg()
                    .path("icons/check.svg")
                    .size_4()
                    .text_color(palette::accent()),
            )
        }))
        .child(label.into())
        .into_any_element()
}

pub(crate) fn menu_caption(text: &'static str) -> AnyElement {
    div()
        .px_1()
        .pt_1()
        .pb_0p5()
        .text_color(palette::faint())
        .child(text)
        .into_any_element()
}

/// A colour square to click.
fn swatch(
    id: impl Into<ElementId>,
    color: u32,
    tooltip: String,
    action: Action,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .size(px(20.0))
        .rounded_sm()
        .bg(rgb(color))
        .border_1()
        .border_color(palette::border())
        .cursor_pointer()
        .hover(|square| square.border_color(palette::accent()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(action, cx))
        .into_any_element()
}

/// A group of controls with its caption under it.
pub(crate) fn group(caption: &'static str, rows: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .justify_between()
        .h_full()
        .px_2()
        .border_r_1()
        .border_color(palette::border())
        .child(div().flex().flex_col().gap_1().children(rows))
        .child(
            div()
                .flex()
                .justify_center()
                .text_xs()
                .text_color(palette::faint())
                .child(caption),
        )
        .into_any_element()
}

pub(crate) fn row(children: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_0p5()
        .children(children)
        .into_any_element()
}

fn size_label(size: f32) -> String {
    if size.fract() == 0.0 {
        format!("{size:.0}")
    } else {
        format!("{size}")
    }
}

/// A gallery entry's label: the shell's words for the fixed four, the
/// document's name for its own.
fn style_label(entry: &GalleryStyle) -> SharedString {
    match entry.preset {
        Some(index) => i18n::text(match index {
            0 => "doc.style.normal",
            1 => "doc.style.heading1",
            2 => "doc.style.heading2",
            _ => "doc.style.heading3",
        })
        .into(),
        None => entry.name.clone().unwrap_or_default().into(),
    }
}

/// The whole ribbon: the tab strip, with where the caret is on its right,
/// and the selected tab's groups.
/// The page the ページ設定 group shows: its setup, which 余白 preset its
/// margins are, and whether Japanese Word's presets are meant.
pub struct PageState {
    pub setup: PageSetup,
    pub preset: Option<MarginPreset>,
    pub japanese: bool,
}

/// What the tabs beyond ホーム show as current.
pub struct TabState {
    /// The page, read only while レイアウト shows it.
    pub page: Option<PageState>,
    /// Whether changes are tracked.
    pub tracking: bool,
    /// Whether the comments pane is open.
    pub comments: bool,
    /// Whether a picture is selected, which 文字列の折り返し acts on.
    pub picture: bool,
    /// What the text, line number and background groups show, read only
    /// while レイアウト is up.
    pub layout: Option<crate::view::word_setup::LayoutState>,
}

pub fn ribbon(
    ribbon: &Ribbon,
    state: &FormatState,
    shown: &TabState,
    position: String,
    dirty: bool,
    viewport: Size<Pixels>,
    cx: &mut Context<DocView>,
) -> AnyElement {
    let tabs = Tab::ALL.map(|tab| {
        let active = tab == ribbon.tab;
        div()
            .id(("ribbon-tab", tab as usize))
            .px_3()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .text_xs()
            .when(active, |element| {
                element.bg(palette::selected()).text_color(palette::text())
            })
            .when(!active, |element| element.text_color(palette::dim()))
            .hover(|element| element.bg(palette::hovered()))
            .child(tab.label())
            .on_click(listener(Action::Tab(tab), cx))
    });
    let groups = match ribbon.tab {
        Tab::Home => home(ribbon, state, cx),
        Tab::Insert => {
            let mut groups = insert(ribbon, cx);
            groups.push(crate::word_page::pictures(shown.picture, cx));
            groups.push(crate::word_page::header_footer(ribbon, cx));
            groups.push(crate::view::word_setup::cover_group(ribbon.popup, cx));
            groups
        }
        Tab::Layout => {
            let mut groups = Vec::new();
            if let Some(page) = &shown.page {
                groups.push(crate::word_page::page_setup(
                    ribbon,
                    &page.setup,
                    page.preset,
                    page.japanese,
                    cx,
                ));
            }
            if let Some(layout) = &shown.layout {
                groups.extend(crate::view::word_setup::layout_groups(
                    ribbon.popup,
                    layout,
                    cx,
                ));
                groups.extend(crate::view::word_setup::background_groups(
                    ribbon.popup,
                    layout,
                    cx,
                ));
            }
            groups.extend(layout(ribbon, state, cx));
            groups
        }
        Tab::Review => crate::word_review::review(shown.tracking, shown.comments, cx),
    };
    div()
        .flex()
        .flex_col()
        .flex_none()
        .bg(palette::panel())
        .border_b_1()
        .border_color(palette::border())
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .px_3()
                .pt_1p5()
                .children(tabs)
                .child(div().flex_1())
                .child(
                    div()
                        .px_1()
                        .text_xs()
                        .text_color(palette::dim())
                        .child(position),
                )
                .when(dirty, |bar| {
                    bar.child(div().size_1p5().mr_1().rounded_full().bg(palette::dim()))
                }),
        )
        .child(
            div()
                .id("ribbon-groups")
                .flex()
                .items_start()
                .h(px(78.0))
                .px_2()
                .py_1p5()
                // Scrolls rather than cuts off: a group past the edge is a
                // command nobody can reach.
                .overflow_x_scroll()
                .children(groups),
        )
        // Behind an open list, over everything else: a click outside the
        // list closes it, and goes no further.
        .children(ribbon.popup.map(|_| {
            deferred(
                anchored().position(point(px(0.0), px(0.0))).child(
                    div()
                        .id("ribbon-backdrop")
                        .w(viewport.width)
                        .h(viewport.height)
                        .occlude()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|view, _event, window, cx| {
                                view.ribbon_action(Action::ClosePopup, window, cx)
                            }),
                        ),
                ),
            )
            .with_priority(1)
        }))
        .into_any_element()
}

// ---- ホーム -------------------------------------------------------------------

fn home(ribbon: &Ribbon, state: &FormatState, cx: &mut Context<DocView>) -> Vec<AnyElement> {
    let open = ribbon.popup;
    let family = state
        .font_family
        .clone()
        .unwrap_or_else(|| DEFAULT_FAMILY.to_string());
    let size = state.font_size.unwrap_or(DEFAULT_SIZE);

    let family_menu = (open == Some(Popup::FontFamily)).then(|| {
        menu_panel("family-menu")
            .w(px(200.0))
            .children(FONT_FAMILIES.iter().enumerate().map(|(index, name)| {
                menu_row(
                    ("family", index),
                    *name,
                    family == *name,
                    Action::FontFamily(name),
                    cx,
                )
            }))
            .into_any_element()
    });
    let size_menu = (open == Some(Popup::FontSize)).then(|| {
        menu_panel("size-menu")
            .w(px(72.0))
            .max_h(px(320.0))
            .overflow_y_scroll()
            .children(FONT_SIZES.iter().enumerate().map(|(index, points)| {
                menu_row(
                    ("size", index),
                    size_label(*points),
                    (size - points).abs() < 0.01,
                    Action::FontSize(*points),
                    cx,
                )
            }))
            .into_any_element()
    });
    let color_menu = (open == Some(Popup::FontColor)).then(|| {
        menu_panel("color-menu")
            .w(px(260.0))
            .child(menu_row(
                "color-auto",
                i18n::text("doc.color.auto"),
                false,
                Action::FontColor("auto".to_string()),
                cx,
            ))
            .child(div().flex().flex_wrap().gap_1().p_1().children(
                TEXT_COLORS.iter().enumerate().map(|(index, color)| {
                    swatch(
                        ("color", index),
                        *color,
                        format!("#{}", hex(*color)),
                        Action::FontColor(hex(*color)),
                        cx,
                    )
                }),
            ))
            .into_any_element()
    });
    let highlight_menu = (open == Some(Popup::Highlight)).then(|| {
        menu_panel("highlight-menu")
            .w(px(200.0))
            .child(menu_row(
                "highlight-none",
                i18n::text("doc.none"),
                false,
                Action::Highlight("none"),
                cx,
            ))
            .child(div().flex().flex_wrap().gap_1().p_1().children(
                HIGHLIGHTS.iter().enumerate().map(|(index, (name, color))| {
                    swatch(
                        ("highlight", index),
                        *color,
                        name.to_string(),
                        Action::Highlight(name),
                        cx,
                    )
                }),
            ))
            .into_any_element()
    });
    let spacing_menu = (open == Some(Popup::LineSpacing)).then(|| {
        menu_panel("spacing-menu")
            .w(px(96.0))
            .children(
                LINE_SPACINGS
                    .iter()
                    .enumerate()
                    .map(|(index, (label, multiple))| {
                        menu_row(
                            ("spacing", index),
                            *label,
                            false,
                            Action::LineSpacing(*multiple),
                            cx,
                        )
                    }),
            )
            .into_any_element()
    });
    let shading_menu = (open == Some(Popup::Shading)).then(|| {
        menu_panel("shading-menu")
            .w(px(150.0))
            .child(menu_row(
                "shading-none",
                i18n::text("doc.none"),
                false,
                Action::Shading(None),
                cx,
            ))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .p_1()
                    .children(SHADINGS.iter().enumerate().map(|(index, color)| {
                        swatch(
                            ("shading", index),
                            *color,
                            format!("#{}", hex(*color)),
                            Action::Shading(Some(hex(*color))),
                            cx,
                        )
                    })),
            )
            .into_any_element()
    });
    let borders_menu = (open == Some(Popup::Borders)).then(|| {
        let borders = [
            ("ribbon.border.bottom", ParagraphBorders::Bottom),
            ("ribbon.border.top", ParagraphBorders::Top),
            ("ribbon.border.box", ParagraphBorders::Box),
            ("ribbon.border.none", ParagraphBorders::None),
        ];
        menu_panel("borders-menu")
            .w(px(140.0))
            .children(
                borders
                    .into_iter()
                    .enumerate()
                    .map(|(index, (key, sides))| {
                        menu_row(
                            ("border", index),
                            i18n::text(key),
                            false,
                            Action::ParagraphBorders(sides),
                            cx,
                        )
                    }),
            )
            .into_any_element()
    });

    let alignment = state.alignment.as_deref().unwrap_or("left");
    let align = |id: &'static str,
                 icon: &'static str,
                 tooltip: &'static str,
                 value: &'static str,
                 cx: &mut Context<DocView>| {
        // Word's `start` and `justify` are the same as these two.
        let active = alignment == value
            || (value == "left" && alignment == "start")
            || (value == "both" && alignment == "justify");
        icon_button(id, icon, tooltip, active, true, Action::Align(value), cx)
    };

    vec![
        group(
            i18n::text("doc.group.font"),
            vec![
                row(vec![
                    dropdown(
                        combo_box(
                            "font-family",
                            family.clone(),
                            132.0,
                            i18n::text("ribbon.font_family"),
                            open == Some(Popup::FontFamily),
                            Popup::FontFamily,
                            cx,
                        ),
                        family_menu,
                    ),
                    div().w_1().into_any_element(),
                    dropdown(
                        combo_box(
                            "font-size",
                            size_label(size),
                            52.0,
                            i18n::text("ribbon.font_size"),
                            open == Some(Popup::FontSize),
                            Popup::FontSize,
                            cx,
                        ),
                        size_menu,
                    ),
                    icon_button(
                        "grow",
                        "icons/a-arrow-up.svg",
                        i18n::text("doc.grow_title"),
                        false,
                        true,
                        Action::StepFontSize(true),
                        cx,
                    ),
                    icon_button(
                        "shrink",
                        "icons/a-arrow-down.svg",
                        i18n::text("doc.shrink_title"),
                        false,
                        true,
                        Action::StepFontSize(false),
                        cx,
                    ),
                ]),
                row(vec![
                    icon_button(
                        "bold",
                        "icons/bold.svg",
                        i18n::text("doc.bold_title"),
                        state.bold,
                        true,
                        Action::Toggle(CharacterFormat::Bold),
                        cx,
                    ),
                    icon_button(
                        "italic",
                        "icons/italic.svg",
                        i18n::text("doc.italic_title"),
                        state.italic,
                        true,
                        Action::Toggle(CharacterFormat::Italic),
                        cx,
                    ),
                    icon_button(
                        "underline",
                        "icons/underline.svg",
                        i18n::text("doc.underline_title"),
                        state.underline,
                        true,
                        Action::Toggle(CharacterFormat::Underline),
                        cx,
                    ),
                    icon_button(
                        "strike",
                        "icons/strikethrough.svg",
                        i18n::text("doc.strike_title"),
                        state.strike,
                        true,
                        Action::Toggle(CharacterFormat::Strikethrough),
                        cx,
                    ),
                    icon_button(
                        "subscript",
                        "icons/subscript.svg",
                        i18n::text("doc.subscript_title"),
                        state.subscript,
                        true,
                        Action::Toggle(CharacterFormat::Subscript),
                        cx,
                    ),
                    icon_button(
                        "superscript",
                        "icons/superscript.svg",
                        i18n::text("doc.superscript_title"),
                        state.superscript,
                        true,
                        Action::Toggle(CharacterFormat::Superscript),
                        cx,
                    ),
                    dropdown(
                        menu_button(
                            "font-color",
                            "icons/baseline.svg",
                            i18n::text("doc.font_color_title"),
                            Some(0xdc2626),
                            open == Some(Popup::FontColor),
                            Popup::FontColor,
                            cx,
                        ),
                        color_menu,
                    ),
                    dropdown(
                        menu_button(
                            "highlight",
                            "icons/highlighter.svg",
                            i18n::text("doc.highlight_title"),
                            Some(0xffff00),
                            open == Some(Popup::Highlight),
                            Popup::Highlight,
                            cx,
                        ),
                        highlight_menu,
                    ),
                    icon_button(
                        "clear-format",
                        "icons/eraser.svg",
                        i18n::text("doc.clear_format_title"),
                        false,
                        true,
                        Action::ClearFormatting,
                        cx,
                    ),
                ]),
            ],
        ),
        group(
            i18n::text("doc.group.paragraph"),
            vec![
                row(vec![
                    icon_button(
                        "bullets",
                        "icons/list.svg",
                        i18n::text("doc.bullets"),
                        state.list == Some(ListKind::Bullet),
                        true,
                        Action::List(true),
                        cx,
                    ),
                    icon_button(
                        "numbering",
                        "icons/list-ordered.svg",
                        i18n::text("doc.numbering"),
                        state.list == Some(ListKind::Decimal),
                        true,
                        Action::List(false),
                        cx,
                    ),
                    icon_button(
                        "indent-less",
                        "icons/indent-decrease.svg",
                        i18n::text("doc.indent_less_title"),
                        false,
                        true,
                        Action::Indent(false),
                        cx,
                    ),
                    icon_button(
                        "indent-more",
                        "icons/indent-increase.svg",
                        i18n::text("doc.indent_more_title"),
                        false,
                        true,
                        Action::Indent(true),
                        cx,
                    ),
                    dropdown(
                        menu_button(
                            "line-spacing",
                            "icons/chevrons-up-down.svg",
                            i18n::text("doc.line_spacing_title"),
                            None,
                            open == Some(Popup::LineSpacing),
                            Popup::LineSpacing,
                            cx,
                        ),
                        spacing_menu,
                    ),
                ]),
                row(vec![
                    align(
                        "align-left",
                        "icons/align-left.svg",
                        i18n::text("doc.align_left_title"),
                        "left",
                        cx,
                    ),
                    align(
                        "align-center",
                        "icons/align-center.svg",
                        i18n::text("doc.center_title"),
                        "center",
                        cx,
                    ),
                    align(
                        "align-right",
                        "icons/align-right.svg",
                        i18n::text("doc.align_right_title"),
                        "right",
                        cx,
                    ),
                    align(
                        "align-both",
                        "icons/align-justify.svg",
                        i18n::text("doc.justify_title"),
                        "both",
                        cx,
                    ),
                    dropdown(
                        menu_button(
                            "shading",
                            "icons/paint-bucket.svg",
                            i18n::text("doc.shading_title"),
                            None,
                            open == Some(Popup::Shading),
                            Popup::Shading,
                            cx,
                        ),
                        shading_menu,
                    ),
                    dropdown(
                        menu_button(
                            "paragraph-borders",
                            "icons/square.svg",
                            i18n::text("ribbon.paragraph_borders"),
                            None,
                            open == Some(Popup::Borders),
                            Popup::Borders,
                            cx,
                        ),
                        borders_menu,
                    ),
                ]),
            ],
        ),
        styles_group(ribbon, state, cx),
        group(
            i18n::text("doc.group.editing"),
            vec![row(vec![
                icon_button(
                    "undo",
                    "icons/undo.svg",
                    i18n::text("doc.undo_title"),
                    false,
                    true,
                    Action::Undo,
                    cx,
                ),
                icon_button(
                    "redo",
                    "icons/redo.svg",
                    i18n::text("doc.redo"),
                    false,
                    true,
                    Action::Redo,
                    cx,
                ),
            ])],
        ),
    ]
}

/// The style gallery: the first few entries as cards, the whole list
/// behind the chevron.
fn styles_group(ribbon: &Ribbon, state: &FormatState, cx: &mut Context<DocView>) -> AnyElement {
    let current = state.style_id.as_deref();
    let default = ribbon.default_style.as_deref();
    let card = |index: usize, entry: &GalleryStyle, cx: &mut Context<DocView>| {
        let active = entry.is_active(current, default);
        let heading = entry.preset.is_some_and(|preset| preset > 0);
        div()
            .id(("style", index))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(72.0))
            .h(px(48.0))
            .px_1()
            .rounded_md()
            .border_1()
            .border_color(if active {
                palette::accent()
            } else {
                palette::border()
            })
            .when(active, |card| card.bg(palette::selected()))
            .cursor_pointer()
            .hover(|card| card.bg(palette::hovered()))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_xs()
            .when(heading, |card| card.font_weight(gpui::FontWeight::BOLD))
            .text_color(palette::text())
            .tooltip(Tooltip::text(style_label(entry)))
            .child(style_label(entry))
            .on_click(listener(Action::Style(entry.id.clone()), cx))
            .into_any_element()
    };
    let mut cards: Vec<AnyElement> = ribbon
        .gallery
        .iter()
        .take(GALLERY_INLINE)
        .enumerate()
        .map(|(index, entry)| card(index, entry, cx))
        .collect();
    let menu = (ribbon.popup == Some(Popup::Styles)).then(|| {
        menu_panel("styles-menu")
            .w(px(220.0))
            .max_h(px(360.0))
            .overflow_y_scroll()
            .children(ribbon.gallery.iter().enumerate().map(|(index, entry)| {
                menu_row(
                    ("style-row", index),
                    style_label(entry),
                    entry.is_active(current, default),
                    Action::Style(entry.id.clone()),
                    cx,
                )
            }))
            .into_any_element()
    });
    cards.push(dropdown(
        div()
            .id("styles-more")
            .flex()
            .items_center()
            .justify_center()
            .w(px(18.0))
            .h(px(48.0))
            .rounded_md()
            .cursor_pointer()
            .hover(|button| button.bg(palette::hovered()))
            .tooltip(Tooltip::text(i18n::text("ribbon.styles_more")))
            .on_click(listener(Action::Popup(Popup::Styles), cx))
            .child(
                svg()
                    .path("icons/chevron-down.svg")
                    .size_3()
                    .text_color(palette::dim()),
            )
            .into_any_element(),
        menu,
    ));
    group(
        i18n::text("doc.group.styles"),
        vec![div()
            .flex()
            .items_center()
            .gap_1()
            .children(cards)
            .into_any_element()],
    )
}

// ---- 挿入 ---------------------------------------------------------------------

fn insert(ribbon: &Ribbon, cx: &mut Context<DocView>) -> Vec<AnyElement> {
    let (columns, rows) = ribbon.grid;
    let picker = (ribbon.popup == Some(Popup::InsertTable)).then(|| {
        let mut cells = Vec::with_capacity(GRID_ROWS);
        for row in 1..=GRID_ROWS {
            let mut line = Vec::with_capacity(GRID_COLUMNS);
            for column in 1..=GRID_COLUMNS {
                let lit = column <= columns && row <= rows;
                line.push(
                    div()
                        .id(("grid", row * GRID_COLUMNS + column))
                        .size(px(16.0))
                        .rounded_sm()
                        .border_1()
                        .border_color(if lit {
                            palette::accent()
                        } else {
                            palette::border()
                        })
                        .when(lit, |cell| cell.bg(palette::selected()))
                        .cursor_pointer()
                        .on_hover(cx.listener(move |view, hovered: &bool, window, cx| {
                            if *hovered {
                                view.ribbon_action(Action::Grid(column, row), window, cx)
                            }
                        }))
                        .on_click(listener(Action::InsertTable(row, column), cx)),
                );
            }
            cells.push(div().flex().gap(px(3.0)).children(line));
        }
        menu_panel("table-menu")
            .p_2()
            .gap(px(3.0))
            .child(div().pb_1().text_color(palette::dim()).child(i18n::format(
                "ribbon.table_size",
                &[("columns", &columns), ("rows", &rows)],
            )))
            .children(cells)
            .into_any_element()
    });
    vec![group(
        i18n::text("doc.group.table"),
        vec![dropdown(
            div()
                .id("insert-table")
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_0p5()
                .w(px(52.0))
                .h(px(48.0))
                .rounded_md()
                .cursor_pointer()
                .when(ribbon.popup == Some(Popup::InsertTable), |button| {
                    button.bg(palette::selected())
                })
                .hover(|button| button.bg(palette::hovered()))
                .tooltip(Tooltip::text(i18n::text("ribbon.insert_table_title")))
                .on_click(listener(Action::Popup(Popup::InsertTable), cx))
                .child(
                    svg()
                        .path("icons/table.svg")
                        .size_5()
                        .text_color(palette::text()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .text_xs()
                        .text_color(palette::text())
                        .child(i18n::text("ribbon.insert_table"))
                        .child(
                            svg()
                                .path("icons/chevron-down.svg")
                                .size_3()
                                .text_color(palette::dim()),
                        ),
                )
                .into_any_element(),
            picker,
        )],
    )]
}

// ---- レイアウト (表ツール) -------------------------------------------------------

fn layout(ribbon: &Ribbon, state: &FormatState, cx: &mut Context<DocView>) -> Vec<AnyElement> {
    let cell = state.table;
    let in_table = cell.is_some();
    let table = |command: TableCommand| Action::Table(command);
    let style_menu = (ribbon.popup == Some(Popup::TableStyle)).then(|| {
        menu_panel("table-style-menu")
            .w(px(160.0))
            .children(TABLE_PRESETS.iter().enumerate().map(
                |(index, (preset, key, header, band))| {
                    div()
                        .id(("table-preset", index))
                        .flex()
                        .items_center()
                        .gap_2()
                        .h(px(28.0))
                        .px_1()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|row| row.bg(palette::hovered()))
                        .on_click(listener(
                            table(TableCommand::StylePreset(preset.to_string())),
                            cx,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .w(px(28.0))
                                .border_1()
                                .border_color(palette::border())
                                .child(div().h(px(5.0)).bg(rgb(*header)))
                                .child(div().h(px(5.0)).bg(rgb(0xffffff)))
                                .child(div().h(px(5.0)).bg(rgb(*band))),
                        )
                        .child(i18n::text(key))
                        .into_any_element()
                },
            ))
            .into_any_element()
    });
    let format_menu = (ribbon.popup == Some(Popup::CellFormat))
        .then(|| cell_format_menu(state, cx))
        .flatten();
    let can_merge = cell.is_some_and(|cell| cell.can_merge);
    let can_unmerge = cell.is_some_and(|cell| cell.can_unmerge);
    vec![
        group(
            i18n::text("ribbon.group.rows"),
            vec![
                row(vec![
                    labeled_button(
                        "row-above",
                        "icons/arrow-up-to-line.svg",
                        i18n::text("ribbon.row_above"),
                        in_table,
                        table(TableCommand::InsertRowAbove),
                        cx,
                    ),
                    labeled_button(
                        "row-below",
                        "icons/arrow-down-to-line.svg",
                        i18n::text("ribbon.row_below"),
                        in_table,
                        table(TableCommand::InsertRowBelow),
                        cx,
                    ),
                ]),
                row(vec![labeled_button(
                    "row-delete",
                    "icons/trash-2.svg",
                    i18n::text("ribbon.row_delete"),
                    in_table,
                    table(TableCommand::DeleteRow),
                    cx,
                )]),
            ],
        ),
        group(
            i18n::text("ribbon.group.columns"),
            vec![
                row(vec![
                    labeled_button(
                        "column-left",
                        "icons/arrow-left-to-line.svg",
                        i18n::text("ribbon.column_left"),
                        in_table,
                        table(TableCommand::InsertColumnLeft),
                        cx,
                    ),
                    labeled_button(
                        "column-right",
                        "icons/arrow-right-to-line.svg",
                        i18n::text("ribbon.column_right"),
                        in_table,
                        table(TableCommand::InsertColumnRight),
                        cx,
                    ),
                ]),
                row(vec![labeled_button(
                    "column-delete",
                    "icons/trash-2.svg",
                    i18n::text("ribbon.column_delete"),
                    in_table,
                    table(TableCommand::DeleteColumn),
                    cx,
                )]),
            ],
        ),
        group(
            i18n::text("doc.group.table"),
            vec![
                row(vec![labeled_button(
                    "table-delete",
                    "icons/trash-2.svg",
                    i18n::text("ribbon.table_delete"),
                    in_table,
                    table(TableCommand::DeleteTable),
                    cx,
                )]),
                row(vec![labeled_button(
                    "table-split",
                    "icons/split.svg",
                    i18n::text("ribbon.table_split"),
                    in_table,
                    table(TableCommand::SplitTable),
                    cx,
                )]),
            ],
        ),
        group(
            i18n::text("ribbon.group.merge"),
            vec![
                row(vec![labeled_button(
                    "merge",
                    "icons/merge.svg",
                    i18n::text("ribbon.merge"),
                    can_merge,
                    table(TableCommand::MergeCells),
                    cx,
                )]),
                row(vec![labeled_button(
                    "unmerge",
                    "icons/split.svg",
                    i18n::text("ribbon.unmerge"),
                    can_unmerge,
                    table(TableCommand::UnmergeCells),
                    cx,
                )]),
            ],
        ),
        group(
            i18n::text("ribbon.group.table_format"),
            vec![
                row(vec![dropdown(
                    popup_button(
                        "table-style",
                        "icons/paintbrush.svg",
                        i18n::text("ribbon.table_style"),
                        in_table,
                        ribbon.popup == Some(Popup::TableStyle),
                        Popup::TableStyle,
                        cx,
                    ),
                    style_menu,
                )]),
                row(vec![dropdown(
                    popup_button(
                        "cell-format",
                        "icons/settings-2.svg",
                        i18n::text("ribbon.cell_format"),
                        in_table,
                        ribbon.popup == Some(Popup::CellFormat),
                        Popup::CellFormat,
                        cx,
                    ),
                    format_menu,
                )]),
            ],
        ),
    ]
}

/// A labelled button that opens a list, for the table-format group.
fn popup_button(
    id: &'static str,
    icon: &'static str,
    label: &'static str,
    enabled: bool,
    open: bool,
    popup: Popup,
    cx: &mut Context<DocView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1p5()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .text_color(if enabled {
            palette::text()
        } else {
            palette::faint()
        })
        .when(open, |button| button.bg(palette::selected()))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(palette::hovered()))
                .on_click(listener(Action::Popup(popup), cx))
        })
        .when(!enabled, |button| {
            button.tooltip(Tooltip::text(i18n::text("ribbon.needs_table")))
        })
        .child(svg().path(icon).size_3p5().text_color(if enabled {
            palette::dim()
        } else {
            palette::faint()
        }))
        .child(label)
        .child(
            svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(palette::dim()),
        )
        .into_any_element()
}

/// jade-ide's セルの書式 popover: the table's and the cell's alignment, the
/// cell's shading, the borders, evening rows and columns out, splitting the
/// cell, and the repeated header row.
fn cell_format_menu(state: &FormatState, cx: &mut Context<DocView>) -> Option<AnyElement> {
    let cell = state.table?;
    let table = |command: TableCommand| Action::Table(command);
    let chip = |id: (&'static str, usize),
                label: SharedString,
                active: bool,
                action: Action,
                cx: &mut Context<DocView>| {
        div()
            .id(id)
            .flex()
            .items_center()
            .h(px(24.0))
            .px_2()
            .rounded_md()
            .border_1()
            .border_color(if active {
                palette::accent()
            } else {
                palette::border()
            })
            .when(active, |chip| chip.bg(palette::selected()))
            .whitespace_nowrap()
            .cursor_pointer()
            .hover(|chip| chip.bg(palette::hovered()))
            .on_click(listener(action, cx))
            .child(label)
            .into_any_element()
    };
    let chips = |children: Vec<AnyElement>| {
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .px_1()
            .pb_1()
            .children(children)
            .into_any_element()
    };
    let aligned = [
        ("left", "doc.align_left"),
        ("center", "doc.center"),
        ("right", "doc.align_right"),
    ];
    let vertical = [
        ("top", "ribbon.top"),
        ("center", "doc.center"),
        ("bottom", "ribbon.bottom"),
    ];
    let splits = [(2, 1), (3, 1), (1, 2), (2, 2)];
    Some(
        menu_panel("cell-format-menu")
            .w(px(248.0))
            .child(menu_caption(i18n::text("ribbon.table_align")))
            .child(chips(
                aligned
                    .iter()
                    .enumerate()
                    .map(|(index, (value, key))| {
                        chip(
                            ("table-align", index),
                            i18n::text(key).into(),
                            false,
                            table(TableCommand::Align(value.to_string())),
                            cx,
                        )
                    })
                    .collect(),
            ))
            .child(menu_caption(i18n::text("ribbon.cell_align")))
            .child(chips(
                vertical
                    .iter()
                    .enumerate()
                    .map(|(index, (value, key))| {
                        chip(
                            ("cell-align", index),
                            i18n::text(key).into(),
                            false,
                            table(TableCommand::CellVerticalAlign(value.to_string())),
                            cx,
                        )
                    })
                    .collect(),
            ))
            .child(menu_caption(i18n::text("ribbon.shading")))
            .child(chips({
                let mut swatches: Vec<AnyElement> = CELL_SHADINGS
                    .iter()
                    .enumerate()
                    .map(|(index, color)| {
                        swatch(
                            ("cell-shading", index),
                            *color,
                            format!("#{}", hex(*color)),
                            table(TableCommand::CellShading(Some(hex(*color)))),
                            cx,
                        )
                    })
                    .collect();
                swatches.push(chip(
                    ("cell-shading-none", 0),
                    i18n::text("doc.none").into(),
                    false,
                    table(TableCommand::CellShading(None)),
                    cx,
                ));
                swatches
            }))
            .child(menu_caption(i18n::text("doc.borders")))
            .child(chips(vec![
                chip(
                    ("borders", 0),
                    i18n::text("ribbon.borders_all").into(),
                    false,
                    table(TableCommand::Borders(TableBorders::All)),
                    cx,
                ),
                chip(
                    ("borders", 1),
                    i18n::text("ribbon.borders_none").into(),
                    false,
                    table(TableCommand::Borders(TableBorders::None)),
                    cx,
                ),
            ]))
            .child(menu_caption(i18n::text("ribbon.size")))
            .child(chips(vec![
                chip(
                    ("distribute", 0),
                    i18n::text("ribbon.distribute_rows").into(),
                    false,
                    table(TableCommand::DistributeRows),
                    cx,
                ),
                chip(
                    ("distribute", 1),
                    i18n::text("ribbon.distribute_columns").into(),
                    false,
                    table(TableCommand::DistributeColumns),
                    cx,
                ),
            ]))
            .child(menu_caption(i18n::text("ribbon.split_cell")))
            .child(chips(
                splits
                    .iter()
                    .enumerate()
                    .map(|(index, (columns, rows))| {
                        chip(
                            ("split-cell", index),
                            i18n::format(
                                "ribbon.split_size",
                                &[("columns", columns), ("rows", rows)],
                            )
                            .into(),
                            false,
                            table(TableCommand::SplitCell {
                                columns: *columns,
                                rows: *rows,
                            }),
                            cx,
                        )
                    })
                    .collect(),
            ))
            .child(menu_row(
                "repeat-header",
                i18n::text("ribbon.repeat_header"),
                cell.repeat_header,
                table(TableCommand::RepeatHeader(!cell.repeat_header)),
                cx,
            ))
            .into_any_element(),
    )
}

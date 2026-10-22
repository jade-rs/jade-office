//! Word for Mac's table tools: the テーブル デザイン and テーブル レイアウト tabs
//! that appear while the caret is in a table, 表のプロパティ (表 / 行 / 列 /
//! セル), 線種とページ罫線と網かけ, セルの挿入 / セルの削除 / セルの分割,
//! 文字列を表にする / 表の解除, 並べ替え and 計算式 — and the mouse in a table:
//! a column or row border dragged, a row, a column, a cell or the whole table
//! selected from its edge, a drag across cells, and 罫線を引く / 消しゴム.
//!
//! What each does is `jade-office-model`'s `WordTables`, which the browser
//! shell runs too.

use gpui::{
    div, prelude::*, px, AnyElement, ClickEvent, Context, CursorStyle, MouseButton, MouseDownEvent,
    MouseMoveEvent, SharedString, Window,
};
use jade_office_model::word::table::{
    AutoFit, BorderSet, BorderTarget, BordersDialog, CellShift, DrawTool, Field, LookOption, Pen,
    PropertiesTab, SortKind as HeadlessTableSortKind, TablePart, TablePropertiesDialog, WordTables,
    MM_PER_PT,
};
use jade_office_model::word::{FormatState, TableCommand, WordSession};

use crate::ribbon::{
    dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup,
};
use crate::tooltip::Tooltip;
use crate::{t, theme, DocumentView};

/// The shading swatches of テーブル デザイン's 網かけ and the borders dialog.
const SHADES: [&str; 10] = [
    "FFFFFF", "F2F2F2", "D9D9D9", "DEEAF6", "D9E2F3", "FBE4D5", "FFF2CC", "E2EFD9", "FCE4EC",
    "EDEDED",
];
/// ペンの色.
const PEN_COLORS: [&str; 10] = [
    "auto", "000000", "7F7F7F", "C00000", "ED7D31", "FFC000", "70AD47", "4472C4", "2F5496",
    "7030A0",
];
/// 表示形式 of 計算式.
const NUMBER_FORMATS: [&str; 7] = ["", "0", "#,##0", "#,##0.00", "0%", "0.00%", "¥#,##0"];
/// 関数の貼り付け.
const FUNCTIONS: [&str; 9] = [
    "SUM", "AVERAGE", "COUNT", "MAX", "MIN", "PRODUCT", "ROUND", "ABS", "INT",
];

pub use jade_office_model::word::table_ui::{
    DialogBox, Separator, SortSlot, TableAction, TableCursor, TableDialog, TableDrag, TableOutcome,
    TableTabState, TableTools as TableUi,
};

/// The pointer over the pages, in gpui's shapes.
pub fn cursor_style(tools: &TableUi) -> CursorStyle {
    match tools.cursor() {
        TableCursor::Text => CursorStyle::IBeam,
        TableCursor::Arrow => CursorStyle::Arrow,
        TableCursor::ResizeColumns => CursorStyle::ResizeLeftRight,
        TableCursor::ResizeRows => CursorStyle::ResizeUpDown,
        TableCursor::Pencil => CursorStyle::Crosshair,
        TableCursor::Eraser => CursorStyle::PointingHand,
    }
}

fn mm(points: f32) -> String {
    format!("{:.1}", points * MM_PER_PT)
}

/// A toggle chip: on shows selected.
fn chip(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when(on, |chip| chip.bg(theme::selected()))
        .cursor_pointer()
        .hover(|chip| chip.bg(theme::hovered()))
        .whitespace_nowrap()
        .child(label.into())
}

/// A checkbox with its label.
fn checkbox(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .whitespace_nowrap()
        .child(
            div()
                .size(px(12.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .when(on, |box_| {
                    box_.bg(theme::accent()).child(
                        gpui::svg()
                            .path("icons/check.svg")
                            .size(px(10.0))
                            .text_color(gpui::white()),
                    )
                }),
        )
        .child(label.into())
}

fn swatch_box(id: impl Into<gpui::ElementId>, color: &str, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(18.0))
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when_some(
            u32::from_str_radix(color.trim_start_matches('#'), 16).ok(),
            |swatch, rgb| swatch.bg(gpui::rgb(rgb)),
        )
        .cursor_pointer()
}

fn label(text: &'static str) -> gpui::Div {
    div()
        .w(px(132.0))
        .flex_none()
        .text_color(theme::text_dim())
        .child(t(text))
}

fn line() -> gpui::Div {
    div().flex().flex_wrap().items_center().gap_1()
}

/// A ribbon button with a label, for the table tabs.
fn text_button(
    id: impl Into<gpui::ElementId>,
    text: &'static str,
    tooltip: &'static str,
    active: bool,
    enabled: bool,
    action: Action,
    cx: &mut Context<DocumentView>,
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
        .whitespace_nowrap()
        .text_color(if enabled {
            theme::text()
        } else {
            theme::text_faint()
        })
        .when(active, |button| button.bg(theme::selected()))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .on_click(listener(action, cx))
        })
        .tooltip(Tooltip::text(t(tooltip)))
        .child(t(text))
        .into_any_element()
}

/// A labelled button with a chevron that opens one of the ribbon's lists.
fn list_button(
    id: &'static str,
    text: &'static str,
    enabled: bool,
    open: bool,
    popup: Popup,
    cx: &mut Context<DocumentView>,
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
        .text_color(if enabled {
            theme::text()
        } else {
            theme::text_faint()
        })
        .when(open, |button| button.bg(theme::selected()))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .on_click(listener(Action::Popup(popup), cx))
        })
        .child(t(text))
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element()
}

fn tables(action: TableAction) -> Action {
    Action::Tables(action)
}

// ---- テーブル デザイン ------------------------------------------------------------

/// The テーブル デザイン tab's groups.
pub fn design_groups(
    open: Option<Popup>,
    tabs: &TableTabState,
    state: &FormatState,
    cx: &mut Context<DocumentView>,
) -> Vec<AnyElement> {
    let in_table = state.table.is_some();
    let look = tabs.look;
    let options: Vec<AnyElement> = [
        (LookOption::HeaderRow, LookOption::FirstColumn),
        (LookOption::TotalRow, LookOption::LastColumn),
        (LookOption::BandedRows, LookOption::BandedColumns),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (left, right))| {
        let check = |option: LookOption, slot: usize, cx: &mut Context<DocumentView>| {
            let on = option.is_on(&look);
            checkbox(("look", slot), t(look_key(option)), on)
                .w(px(96.0))
                .text_size(px(11.0))
                .when(in_table, |check| {
                    check.on_click(listener(tables(TableAction::Look(option, !on)), cx))
                })
                .into_any_element()
        };
        div()
            .flex()
            .gap_2()
            .h(px(15.0))
            .child(check(left, index * 2, cx))
            .child(check(right, index * 2 + 1, cx))
            .into_any_element()
    })
    .collect();

    let styles = &tabs.styles;
    let current = tabs.style_id.clone();
    let tile = |index: usize,
                style: &jade_office_model::word::table::TableStyleChoice,
                cx: &mut Context<DocumentView>| {
        let (header, band, border) = style.preview.clone();
        let color = |value: Option<String>, fallback: u32| {
            value
                .and_then(|value| u32::from_str_radix(&value, 16).ok())
                .map(gpui::rgb)
                .unwrap_or(gpui::rgb(fallback))
        };
        let border = color(border, 0xbfbfbf);
        let header_fill = color(header, 0xffffff);
        let band_fill = color(band, 0xffffff);
        let on = current.as_deref() == Some(style.id.as_str());
        div()
            .id(("table-style", index))
            .flex()
            .flex_col()
            .w(px(40.0))
            .h(px(32.0))
            .p(px(2.0))
            .rounded_sm()
            .border_1()
            .border_color(if on { theme::accent() } else { theme::border() })
            .bg(gpui::white())
            .when(in_table, |tile| {
                tile.cursor_pointer()
                    .hover(|tile| tile.border_color(theme::accent()))
                    .on_click(listener(
                        tables(TableAction::Style(Some(style.id.clone()))),
                        cx,
                    ))
            })
            .tooltip(Tooltip::text(style.name.clone()))
            .children((0..4).map(move |row| {
                div()
                    .flex_1()
                    .border_b_1()
                    .border_color(border)
                    .bg(if row == 0 {
                        header_fill
                    } else if row % 2 == 1 {
                        band_fill
                    } else {
                        gpui::rgb(0xffffff)
                    })
            }))
            .into_any_element()
    };
    let inline: Vec<AnyElement> = styles
        .iter()
        .take(6)
        .enumerate()
        .map(|(index, style)| tile(index, style, cx))
        .collect();
    let menu = (open == Some(Popup::TableStyles)).then(|| {
        menu_panel("table-styles-menu")
            .w(px(260.0))
            .child(menu_row(
                "table-style-none",
                t("table.style_clear"),
                current.is_none(),
                tables(TableAction::Style(None)),
                cx,
            ))
            .children(styles.iter().enumerate().map(|(index, style)| {
                menu_row(
                    ("table-style-row", index),
                    style.name.clone(),
                    current.as_deref() == Some(style.id.as_str()),
                    tables(TableAction::Style(Some(style.id.clone()))),
                    cx,
                )
            }))
            .into_any_element()
    });

    let shading_menu = (open == Some(Popup::TableShading)).then(|| {
        menu_panel("table-shading-menu")
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .w(px(130.0))
                    .p_1()
                    .children(SHADES.iter().enumerate().map(|(index, color)| {
                        swatch_box(("shade", index), color, false)
                            .on_click(listener(tables(TableAction::Shading(Some(color))), cx))
                            .into_any_element()
                    })),
            )
            .child(menu_row(
                "shade-none",
                t("table.no_color"),
                false,
                tables(TableAction::Shading(None)),
                cx,
            ))
            .into_any_element()
    });
    let pen = tabs.pen.clone();
    let style_menu = (open == Some(Popup::PenStyle)).then(|| {
        menu_panel("pen-style-menu")
            .children(Pen::STYLES.iter().enumerate().map(|(index, style)| {
                menu_row(
                    ("pen-style", index),
                    t(line_key(style)),
                    pen.style == *style,
                    tables(TableAction::PenStyle(style)),
                    cx,
                )
            }))
            .into_any_element()
    });
    let width_menu = (open == Some(Popup::PenWidth)).then(|| {
        menu_panel("pen-width-menu")
            .children(Pen::WIDTHS.iter().enumerate().map(|(index, width)| {
                menu_row(
                    ("pen-width", index),
                    format!("{width} pt"),
                    (pen.width - width).abs() < 0.01,
                    tables(TableAction::PenWidth(*width)),
                    cx,
                )
            }))
            .into_any_element()
    });
    let color_menu = (open == Some(Popup::PenColor)).then(|| {
        menu_panel("pen-color-menu")
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .w(px(130.0))
                    .p_1()
                    .children(PEN_COLORS.iter().enumerate().map(|(index, color)| {
                        let shown = if *color == "auto" { "000000" } else { color };
                        swatch_box(("pen-color", index), shown, pen.color == *color)
                            .on_click(listener(tables(TableAction::PenColor(color)), cx))
                            .into_any_element()
                    })),
            )
            .into_any_element()
    });
    let sets_menu = (open == Some(Popup::BorderSets)).then(|| {
        menu_panel("border-sets-menu")
            .children(BorderSet::ALL.iter().enumerate().map(|(index, set)| {
                menu_row(
                    ("border-set", index),
                    t(border_key(*set)),
                    false,
                    tables(TableAction::BorderSet(*set)),
                    cx,
                )
            }))
            .child(menu_row(
                "border-dialog",
                t("table.borders_dialog"),
                false,
                tables(TableAction::Borders),
                cx,
            ))
            .into_any_element()
    });
    vec![
        group(t("table.group.style_options"), options),
        group(
            t("table.group.styles"),
            vec![row(inline
                .into_iter()
                .chain([dropdown(
                    list_button(
                        "table-styles",
                        "table.more_styles",
                        in_table,
                        open == Some(Popup::TableStyles),
                        Popup::TableStyles,
                        cx,
                    ),
                    menu,
                )])
                .collect())],
        ),
        group(
            t("table.group.borders"),
            vec![
                row(vec![
                    dropdown(
                        list_button(
                            "table-shading",
                            "table.shading",
                            in_table,
                            open == Some(Popup::TableShading),
                            Popup::TableShading,
                            cx,
                        ),
                        shading_menu,
                    ),
                    dropdown(
                        list_button(
                            "pen-style",
                            "table.pen_style",
                            in_table,
                            open == Some(Popup::PenStyle),
                            Popup::PenStyle,
                            cx,
                        ),
                        style_menu,
                    ),
                    dropdown(
                        list_button(
                            "pen-width",
                            "table.pen_width",
                            in_table,
                            open == Some(Popup::PenWidth),
                            Popup::PenWidth,
                            cx,
                        ),
                        width_menu,
                    ),
                ]),
                row(vec![
                    dropdown(
                        list_button(
                            "pen-color",
                            "table.pen_color",
                            in_table,
                            open == Some(Popup::PenColor),
                            Popup::PenColor,
                            cx,
                        ),
                        color_menu,
                    ),
                    dropdown(
                        list_button(
                            "border-sets",
                            "table.border_sets",
                            in_table,
                            open == Some(Popup::BorderSets),
                            Popup::BorderSets,
                            cx,
                        ),
                        sets_menu,
                    ),
                    text_button(
                        "pen-tool",
                        "table.draw",
                        "table.draw_title",
                        tabs.tool == Some(DrawTool::Pencil),
                        true,
                        tables(TableAction::Tool(DrawTool::Pencil)),
                        cx,
                    ),
                ]),
            ],
        ),
    ]
}

fn look_key(option: LookOption) -> &'static str {
    match option {
        LookOption::HeaderRow => "table.look.header_row",
        LookOption::TotalRow => "table.look.total_row",
        LookOption::BandedRows => "table.look.banded_rows",
        LookOption::FirstColumn => "table.look.first_column",
        LookOption::LastColumn => "table.look.last_column",
        LookOption::BandedColumns => "table.look.banded_columns",
    }
}

fn line_key(style: &str) -> &'static str {
    jade_office_model::word::borders::style_label(style)
}

fn border_key(set: BorderSet) -> &'static str {
    match set {
        BorderSet::Bottom => "table.border.bottom",
        BorderSet::Top => "table.border.top",
        BorderSet::Left => "table.border.left",
        BorderSet::Right => "table.border.right",
        BorderSet::None => "table.border.none",
        BorderSet::All => "table.border.all",
        BorderSet::Outside => "table.border.outside",
        BorderSet::Inside => "table.border.inside",
        BorderSet::InsideHorizontal => "table.border.inside_h",
        BorderSet::InsideVertical => "table.border.inside_v",
    }
}

// ---- テーブル レイアウト ------------------------------------------------------------

/// The テーブル レイアウト tab's groups, around the ribbon's own row, column,
/// merge and format groups.
/// The groups before and after the ribbon's own row, column, merge and
/// format groups, which the shell puts between them.
pub fn layout_groups(
    open: Option<Popup>,
    tabs: &TableTabState,
    state: &FormatState,
    cx: &mut Context<DocumentView>,
) -> (Vec<AnyElement>, Vec<AnyElement>) {
    let in_table = state.table.is_some();
    let select_menu = (open == Some(Popup::TableSelect)).then(|| {
        menu_panel("table-select-menu")
            .children(
                [
                    (TablePart::Cell, "table.select.cell"),
                    (TablePart::Column, "table.select.column"),
                    (TablePart::Row, "table.select.row"),
                    (TablePart::Table, "table.select.table"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (part, key))| {
                    menu_row(
                        ("table-select", index),
                        t(key),
                        false,
                        tables(TableAction::Select(part)),
                        cx,
                    )
                }),
            )
            .into_any_element()
    });
    let table_group = group(
        t("table.group.table"),
        vec![
            row(vec![dropdown(
                list_button(
                    "table-select",
                    "table.select",
                    in_table,
                    open == Some(Popup::TableSelect),
                    Popup::TableSelect,
                    cx,
                ),
                select_menu,
            )]),
            row(vec![text_button(
                "table-properties",
                "table.properties",
                "table.properties_title",
                false,
                in_table,
                tables(TableAction::Properties(PropertiesTab::Table)),
                cx,
            )]),
        ],
    );
    let draw_group = group(
        t("table.group.draw"),
        vec![
            row(vec![text_button(
                "draw-table",
                "table.draw",
                "table.draw_title",
                tabs.tool == Some(DrawTool::Pencil),
                true,
                tables(TableAction::Tool(DrawTool::Pencil)),
                cx,
            )]),
            row(vec![text_button(
                "eraser",
                "table.eraser",
                "table.eraser_title",
                tabs.tool == Some(DrawTool::Eraser),
                true,
                tables(TableAction::Tool(DrawTool::Eraser)),
                cx,
            )]),
        ],
    );
    let cells_group = group(
        t("table.group.cells"),
        vec![
            row(vec![text_button(
                "insert-cells",
                "table.insert_cells",
                "table.insert_cells_title",
                false,
                in_table,
                tables(TableAction::InsertCells),
                cx,
            )]),
            row(vec![
                text_button(
                    "delete-cells",
                    "table.delete_cells",
                    "table.delete_cells_title",
                    false,
                    in_table,
                    tables(TableAction::DeleteCells),
                    cx,
                ),
                text_button(
                    "split-cells",
                    "table.split_cells",
                    "table.split_cells_title",
                    false,
                    in_table,
                    tables(TableAction::SplitCells),
                    cx,
                ),
            ]),
        ],
    );
    let autofit_menu = (open == Some(Popup::AutoFit)).then(|| {
        menu_panel("autofit-menu")
            .children(
                [
                    (AutoFit::Contents, "table.autofit.contents"),
                    (AutoFit::Window, "table.autofit.window"),
                    (AutoFit::Fixed, "table.autofit.fixed"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (fit, key))| {
                    menu_row(
                        ("autofit", index),
                        t(key),
                        false,
                        tables(TableAction::AutoFit(fit)),
                        cx,
                    )
                }),
            )
            .into_any_element()
    });
    let (width, height) = tabs.cell_size;
    let stepper = |id: &'static str,
                   caption: &'static str,
                   value: f32,
                   grow: TableAction,
                   shrink: TableAction,
                   cx: &mut Context<DocumentView>| {
        div()
            .flex()
            .items_center()
            .gap_1()
            .text_xs()
            .child(
                div()
                    .w(px(24.0))
                    .text_color(theme::text_dim())
                    .child(t(caption)),
            )
            .child(
                div()
                    .w(px(56.0))
                    .px_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme::border())
                    .child(format!("{} mm", mm(value))),
            )
            .child(text_button(
                (id, 0usize),
                "table.step_down",
                "table.step_down",
                false,
                in_table,
                tables(shrink),
                cx,
            ))
            .child(text_button(
                (id, 1usize),
                "table.step_up",
                "table.step_up",
                false,
                in_table,
                tables(grow),
                cx,
            ))
            .into_any_element()
    };
    let size_group = group(
        t("table.group.cell_size"),
        vec![
            row(vec![
                dropdown(
                    list_button(
                        "autofit",
                        "table.autofit",
                        in_table,
                        open == Some(Popup::AutoFit),
                        Popup::AutoFit,
                        cx,
                    ),
                    autofit_menu,
                ),
                stepper(
                    "row-height",
                    "table.height",
                    height,
                    TableAction::StepHeight(true),
                    TableAction::StepHeight(false),
                    cx,
                ),
            ]),
            row(vec![
                text_button(
                    "distribute-rows",
                    "table.distribute_rows",
                    "table.distribute_rows",
                    false,
                    in_table,
                    Action::Table(TableCommand::DistributeRows),
                    cx,
                ),
                stepper(
                    "column-width",
                    "table.width",
                    width,
                    TableAction::StepWidth(true),
                    TableAction::StepWidth(false),
                    cx,
                ),
                text_button(
                    "distribute-columns",
                    "table.distribute_columns",
                    "table.distribute_columns",
                    false,
                    in_table,
                    Action::Table(TableCommand::DistributeColumns),
                    cx,
                ),
            ]),
        ],
    );
    // 配置: nine buttons, top-left to bottom-right.
    let grid: Vec<AnyElement> = ["top", "center", "bottom"]
        .into_iter()
        .enumerate()
        .map(|(row_index, vertical)| {
            row(["left", "center", "right"]
                .into_iter()
                .enumerate()
                .map(|(column, horizontal)| {
                    let icon = match horizontal {
                        "left" => "icons/align-left.svg",
                        "center" => "icons/align-center.svg",
                        _ => "icons/align-right.svg",
                    };
                    div()
                        .id(("cell-align", row_index * 3 + column))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(16.0))
                        .rounded_sm()
                        .when(in_table, |button| {
                            button
                                .cursor_pointer()
                                .hover(|button| button.bg(theme::hovered()))
                                .on_click(listener(
                                    tables(TableAction::Align(horizontal, vertical)),
                                    cx,
                                ))
                        })
                        .tooltip(Tooltip::text(t(match (vertical, horizontal) {
                            ("top", "left") => "table.align.top_left",
                            ("top", "center") => "table.align.top_center",
                            ("top", _) => "table.align.top_right",
                            ("center", "left") => "table.align.center_left",
                            ("center", "center") => "table.align.center",
                            ("center", _) => "table.align.center_right",
                            (_, "left") => "table.align.bottom_left",
                            (_, "center") => "table.align.bottom_center",
                            _ => "table.align.bottom_right",
                        })))
                        .child(
                            gpui::svg()
                                .path(icon)
                                .size(px(12.0))
                                .text_color(if in_table {
                                    theme::text()
                                } else {
                                    theme::text_faint()
                                }),
                        )
                        .into_any_element()
                })
                .collect())
        })
        .collect();
    let direction_menu = (open == Some(Popup::TextDirection)).then(|| {
        let current = tabs.direction.clone();
        menu_panel("text-direction-menu")
            .children(
                [
                    (None, "lrTb", "table.direction.horizontal"),
                    (Some("tbRlV"), "tbRlV", "table.direction.vertical"),
                    (Some("tbRl"), "tbRl", "table.direction.turn_right"),
                    (Some("btLr"), "btLr", "table.direction.bottom_up"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (value, token, key))| {
                    menu_row(
                        ("text-direction", index),
                        t(key),
                        current == token,
                        tables(TableAction::TextDirection(value)),
                        cx,
                    )
                }),
            )
            .into_any_element()
    });
    let align_group = group(
        t("table.group.alignment"),
        vec![div()
            .flex()
            .gap_2()
            .child(div().flex().flex_col().children(grid))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(dropdown(
                        list_button(
                            "text-direction",
                            "table.text_direction",
                            in_table,
                            open == Some(Popup::TextDirection),
                            Popup::TextDirection,
                            cx,
                        ),
                        direction_menu,
                    ))
                    .child(text_button(
                        "cell-margins",
                        "table.cell_margins",
                        "table.cell_margins_title",
                        false,
                        in_table,
                        tables(TableAction::CellMargins),
                        cx,
                    )),
            )
            .into_any_element()],
    );
    let repeat = state.table.is_some_and(|cell| cell.repeat_header);
    let data_group = group(
        t("table.group.data"),
        vec![
            row(vec![
                text_button(
                    "sort",
                    "table.sort",
                    "table.sort_title",
                    false,
                    in_table,
                    tables(TableAction::Sort),
                    cx,
                ),
                text_button(
                    "repeat-header",
                    "table.repeat_header",
                    "table.repeat_header",
                    repeat,
                    in_table,
                    Action::Table(TableCommand::RepeatHeader(!repeat)),
                    cx,
                ),
                text_button(
                    "to-text",
                    "table.to_text",
                    "table.to_text_title",
                    false,
                    in_table,
                    tables(TableAction::ToText),
                    cx,
                ),
            ]),
            row(vec![
                text_button(
                    "formula",
                    "table.formula",
                    "table.formula_title",
                    false,
                    in_table,
                    tables(TableAction::Formula),
                    cx,
                ),
                text_button(
                    "update-formulas",
                    "table.update_formulas",
                    "table.update_formulas_title",
                    false,
                    true,
                    tables(TableAction::UpdateFormulas),
                    cx,
                ),
            ]),
        ],
    );
    (
        vec![table_group, draw_group],
        vec![cells_group, size_group, align_group, data_group],
    )
}

/// 挿入 ▸ 表's list, under the grid: 罫線を引く and 文字列を表にする.
pub fn insert_menu_rows(cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    vec![
        menu_row(
            "menu-draw-table",
            t("table.draw"),
            false,
            tables(TableAction::Tool(DrawTool::Pencil)),
            cx,
        ),
        menu_row(
            "menu-to-table",
            t("table.to_table"),
            false,
            tables(TableAction::ToTable),
            cx,
        ),
    ]
}

// ---- what the tools do ---------------------------------------------------------

impl DocumentView {
    /// A table control was clicked.
    pub(crate) fn table_action(&mut self, action: TableAction, cx: &mut Context<Self>) {
        let outcome = self.tables.action(&mut self.document, action);
        // A dialog takes the keyboard from the find bar.
        if self.tables.dialog.is_some() {
            self.find_bar = None;
        }
        self.table_outcome(outcome, cx);
    }

    /// What a table action, a dialog or the mouse did, shown.
    pub(crate) fn table_outcome(&mut self, outcome: TableOutcome, cx: &mut Context<Self>) {
        match outcome {
            TableOutcome::NotMine | TableOutcome::Redraw => cx.notify(),
            TableOutcome::Edited(result) => self.table_done(result, cx),
            TableOutcome::Selected(result) => self.table_selected(result, cx),
        }
    }

    /// The tail of a table command: the caret where it went, every page
    /// re-read.
    pub(crate) fn table_done(
        &mut self,
        result: Result<Option<(usize, usize)>, String>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Some(caret)) => self.ribbon_moved(Ok(caret), cx),
            Ok(None) => {
                self.dirty = true;
                self.table_status = None;
                if self.document.selection().is_none() {
                    self.caret = self
                        .document
                        .place_caret(self.insert_at.0, self.insert_at.1);
                }
                self.rebuild_all_images();
                cx.notify();
            }
            Err(error) => {
                self.table_status = Some(SharedString::from(error));
                cx.notify();
            }
        }
    }

    /// After cells were selected: the selection's end is where the caret
    /// is, and the pages show the cells highlighted.
    fn table_selected(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                if let Some((_, focus)) = self.document.selection() {
                    self.insert_at = focus;
                }
                self.selection_anchor = None;
                self.table_status = None;
                self.rebuild_all_images();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// OK in the open dialog.
    pub(crate) fn apply_table_dialog(&mut self, cx: &mut Context<Self>) {
        let outcome = self.tables.apply_dialog(&mut self.document);
        self.table_outcome(outcome, cx);
    }

    /// Keys while a table dialog is open: Enter is OK, Escape cancels,
    /// Backspace edits the focused box and other text types into it.
    pub(crate) fn handle_table_dialog_key(
        &mut self,
        event: &gpui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.platform || keystroke.modifiers.control {
            return false;
        }
        let Some(outcome) = self.tables.dialog_key(
            &mut self.document,
            keystroke.key.as_str(),
            keystroke.key_char.as_deref(),
        ) else {
            return false;
        };
        self.table_outcome(outcome, cx);
        true
    }

    // ---- the mouse ---------------------------------------------------------

    /// The pointer went down at (`x`, `document_y`) on `page`: a border to
    /// drag, a row, column, cell or table to select, or the pencil or the
    /// eraser at work. Returns whether the table took the click.
    pub(crate) fn table_mouse_down(
        &mut self,
        page: usize,
        x: f64,
        document_y: f64,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let outcome =
            self.tables
                .mouse_down(&mut self.document, page, (x, document_y), event.click_count);
        if outcome == TableOutcome::NotMine {
            return false;
        }
        self.drag_anchor = None;
        self.table_outcome(outcome, cx);
        true
    }

    /// The pointer moved with the button down: a border or the pencil
    /// follows it; a drag from one cell into another selects whole cells.
    pub(crate) fn table_mouse_drag(
        &mut self,
        page: usize,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let point = self.page_point(page, event.position);
        let outcome = self
            .tables
            .mouse_drag(&mut self.document, point, self.drag_anchor);
        if outcome == TableOutcome::NotMine {
            return false;
        }
        if matches!(outcome, TableOutcome::Selected(Ok(()))) {
            if let Some((x, y)) = point {
                if let Some(focus) = self.document.position_at(x, y) {
                    self.insert_at = focus;
                }
            }
            self.rebuild_all_images();
            cx.notify();
            return true;
        }
        self.table_outcome(outcome, cx);
        true
    }

    /// The button came up: a dragged border lands, a pencil stroke draws.
    pub(crate) fn table_mouse_up(&mut self, cx: &mut Context<Self>) {
        let outcome = self.tables.mouse_up(&mut self.document, |document, x, y| {
            document.position_at(x, y)
        });
        if outcome != TableOutcome::NotMine {
            self.table_outcome(outcome, cx);
        }
    }

    /// The pointer moved without a button: what it is over decides its shape.
    pub(crate) fn table_hover(
        &mut self,
        page: usize,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        let point = self.page_point(page, event.position);
        if self.tables.hover(&self.document, point) {
            cx.notify();
        }
    }

    /// A window point on `page` in layout pixels: x on the page, y down
    /// the document.
    fn page_point(&self, page: usize, position: gpui::Point<gpui::Pixels>) -> Option<(f64, f64)> {
        let bounds = self.page_bounds.borrow().get(page).copied()??;
        let zoom = f64::from(self.zoom);
        let x = f64::from(position.x - bounds.origin.x) / zoom;
        let y = f64::from(position.y - bounds.origin.y) / zoom;
        Some((x, self.document.document_y(page, y)))
    }

    /// The handle at the caret table's top-left corner, as Word shows it:
    /// a click on it selects the table.
    fn table_handle(&self, page: usize) -> Option<gpui::Div> {
        self.format.as_ref().and_then(|(_, state)| state.table)?;
        let table_id = self
            .document
            .cell_id_for_paragraph(self.insert_at.0)
            .and_then(|id| self.document.cell_layout(id))
            .map(|cell| cell.table_id)?;
        let geometry = self.document.table_geometry(table_id)?;
        let (on, top) = self.document.page_and_offset(geometry.top());
        if on != page {
            return None;
        }
        let zoom = self.zoom;
        Some(
            div()
                .absolute()
                .left(px((geometry.left() as f32 - 20.0) * zoom))
                .top(px((top as f32 - 20.0) * zoom))
                .size(px(12.0 * zoom))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(theme::text_dim())
                .bg(theme::panel())
                .text_size(px(9.0 * zoom))
                .text_color(theme::text_dim())
                .child("+"),
        )
    }

    /// What a drag in a table shows over `page`: the guide where a border
    /// will land, the rectangle or line the pencil draws — or, with no drag,
    /// the caret table's handle.
    pub(crate) fn table_overlay(&self, page: usize) -> Option<impl IntoElement> {
        let Some((drag, drag_page, from, to)) = self.tables.drag else {
            return self.table_handle(page);
        };
        let zoom = self.zoom;
        let local = |y: f64| {
            let (on, offset) = self.document.page_and_offset(y);
            (on, offset as f32)
        };
        let (from_page, from_y) = local(from.1);
        let (_, to_y) = local(to.1);
        if from_page != page || drag_page != page {
            return None;
        }
        let guide = match drag {
            TableDrag::Column { table_id, .. } => {
                let geometry = self.document.table_geometry(table_id)?;
                let (top_page, top) = local(geometry.top());
                let bottom = geometry.rows.last().map_or(geometry.top(), |row| row.1);
                let (_, bottom) = local(bottom);
                let top = if top_page == page { top } else { 0.0 };
                div()
                    .absolute()
                    .left(px(to.0 as f32 * zoom))
                    .top(px(top * zoom))
                    .w(px(1.0))
                    .h(px((bottom - top).max(4.0) * zoom))
                    .bg(theme::accent())
            }
            TableDrag::Row { table_id, .. } => {
                let geometry = self.document.table_geometry(table_id)?;
                div()
                    .absolute()
                    .left(px(geometry.left() as f32 * zoom))
                    .top(px(to_y * zoom))
                    .w(px((geometry.right() - geometry.left()) as f32 * zoom))
                    .h(px(1.0))
                    .bg(theme::accent())
            }
            TableDrag::Stroke => {
                let (left, right) = (from.0.min(to.0) as f32, from.0.max(to.0) as f32);
                let (top, bottom) = (from_y.min(to_y), from_y.max(to_y));
                div()
                    .absolute()
                    .left(px(left * zoom))
                    .top(px(top * zoom))
                    .w(px(((right - left) * zoom).max(1.0)))
                    .h(px(((bottom - top) * zoom).max(1.0)))
                    .border_1()
                    .border_color(theme::accent())
            }
        };
        Some(guide)
    }

    // ---- the dialogs -------------------------------------------------------

    /// The open table dialog, over everything.
    pub(crate) fn table_dialog_element(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let dialog = self.tables.dialog.as_ref()?;
        let body = match dialog {
            TableDialog::Properties(properties) => properties_body(properties, cx),
            TableDialog::Borders(borders) => borders_body(borders, cx),
            TableDialog::Cells { delete, shift } => cells_body(*delete, *shift, cx),
            TableDialog::Split {
                columns,
                rows,
                focus,
            } => split_body(columns, rows, *focus, cx),
            TableDialog::ToTable {
                separator,
                other,
                columns,
                focus,
            } => to_table_body(*separator, other, columns, *focus, cx),
            TableDialog::ToText { separator, other } => to_text_body(*separator, other, cx),
            TableDialog::Sort {
                keys,
                header,
                headings,
            } => sort_body(keys, *header, headings, cx),
            TableDialog::Formula { formula, format } => formula_body(formula, format, cx),
        };
        let width = match dialog {
            TableDialog::Properties(_) | TableDialog::Borders(_) => 520.0,
            _ => 380.0,
        };
        Some(
            div()
                .id("table-dialog-backdrop")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000055))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("table-dialog")
                        .w(px(width))
                        .max_h(px(620.0))
                        .overflow_y_scroll()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .rounded_md()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::panel())
                        .shadow_lg()
                        .text_xs()
                        .text_color(theme::text())
                        .child(div().text_sm().child(t(dialog.title_key())))
                        .child(body)
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    chip("table-dialog-cancel", t("format.cancel"), false)
                                        .on_click(cx.listener(
                                            |view, _: &ClickEvent, _: &mut Window, cx| {
                                                view.tables.dialog = None;
                                                cx.notify();
                                            },
                                        )),
                                )
                                .child(chip("table-dialog-ok", t("format.ok"), true).on_click(
                                    cx.listener(|view, _: &ClickEvent, _: &mut Window, cx| {
                                        view.apply_table_dialog(cx)
                                    }),
                                )),
                        ),
                ),
        )
    }
}

/// A click that edits the open dialog.
fn edit(
    cx: &mut Context<DocumentView>,
    change: impl Fn(&mut TableDialog) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static {
    // `cx.listener`, without its borrow of `cx` in the returned type.
    let view = cx.weak_entity();
    move |_: &ClickEvent, _: &mut Window, cx: &mut gpui::App| {
        view.update(cx, |view, cx| {
            if let Some(dialog) = view.tables.dialog.as_mut() {
                change(dialog);
            }
            cx.notify();
        })
        .ok();
    }
}

/// A click that edits 表のプロパティ.
fn props(
    cx: &mut Context<DocumentView>,
    change: impl Fn(&mut TablePropertiesDialog) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static {
    edit(cx, move |dialog| {
        if let TableDialog::Properties(properties) = dialog {
            change(properties);
        }
    })
}

/// A number box of 表のプロパティ: its value, its unit, and ▼▲.
fn number_box(
    dialog: &TablePropertiesDialog,
    field: Field,
    unit: &'static str,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let focused = dialog.focus == Some(field);
    let id = format!("{field:?}");
    div()
        .flex()
        .items_center()
        .gap_0p5()
        .child(
            div()
                .id(SharedString::from(format!("{id}-box")))
                .w(px(54.0))
                .px_1()
                .rounded_sm()
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .bg(theme::window())
                .cursor_pointer()
                .child(dialog.value(field).to_string())
                .on_click(props(cx, move |dialog| dialog.focus = Some(field))),
        )
        .child(div().text_color(theme::text_dim()).child(unit))
        .child(
            chip(SharedString::from(format!("{id}-down")), "▼", false)
                .on_click(props(cx, move |dialog| dialog.step(field, -0.5))),
        )
        .child(
            chip(SharedString::from(format!("{id}-up")), "▲", false)
                .on_click(props(cx, move |dialog| dialog.step(field, 0.5))),
        )
        .into_any_element()
}

fn properties_body(dialog: &TablePropertiesDialog, cx: &mut Context<DocumentView>) -> AnyElement {
    let tabs = PropertiesTab::ALL
        .iter()
        .enumerate()
        .map(|(index, tab)| {
            let tab = *tab;
            chip(
                ("properties-tab", index),
                t(match tab {
                    PropertiesTab::Table => "table.tab.table",
                    PropertiesTab::Row => "table.tab.row",
                    PropertiesTab::Column => "table.tab.column",
                    PropertiesTab::Cell => "table.tab.cell",
                }),
                dialog.tab == tab,
            )
            .on_click(props(cx, move |dialog| {
                dialog.tab = tab;
                dialog.focus = None;
            }))
            .into_any_element()
        })
        .collect::<Vec<_>>();
    let mm_unit = "mm";
    let body = match dialog.tab {
        PropertiesTab::Table => {
            let mut body = div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    line()
                        .child(
                            checkbox("width-on", t("table.prop.width"), dialog.width_on)
                                .on_click(props(cx, |dialog| dialog.width_on = !dialog.width_on)),
                        )
                        .child(number_box(
                            dialog,
                            Field::TableWidth,
                            if dialog.width_percent { "%" } else { mm_unit },
                            cx,
                        ))
                        .child(
                            div()
                                .text_color(theme::text_dim())
                                .child(t("table.prop.unit")),
                        )
                        .child(
                            chip("unit-mm", "mm", !dialog.width_percent)
                                .on_click(props(cx, |dialog| dialog.width_percent = false)),
                        )
                        .child(
                            chip("unit-percent", "%", dialog.width_percent)
                                .on_click(props(cx, |dialog| dialog.width_percent = true)),
                        ),
                )
                .child(
                    line().child(label("table.prop.alignment")).children(
                        [
                            ("left", "table.prop.left"),
                            ("center", "table.prop.center"),
                            ("right", "table.prop.right"),
                        ]
                        .into_iter()
                        .map(|(value, key)| {
                            chip(
                                SharedString::from(format!("table-align-{value}")),
                                t(key),
                                dialog.alignment == value,
                            )
                            .on_click(props(cx, move |dialog| {
                                dialog.alignment = value.to_string()
                            }))
                        }),
                    ),
                )
                .child(line().child(label("table.prop.indent")).child(number_box(
                    dialog,
                    Field::Indent,
                    mm_unit,
                    cx,
                )))
                .child(
                    line()
                        .child(label("table.prop.wrapping"))
                        .child(
                            chip("wrap-none", t("table.prop.wrap_none"), !dialog.floating)
                                .on_click(props(cx, |dialog| dialog.floating = false)),
                        )
                        .child(
                            chip("wrap-around", t("table.prop.wrap_around"), dialog.floating)
                                .on_click(props(cx, |dialog| dialog.floating = true)),
                        )
                        .child(
                            chip(
                                "position-open",
                                t("table.prop.position"),
                                dialog.position_open,
                            )
                            .on_click(props(cx, |dialog| {
                                dialog.position_open = !dialog.position_open;
                                dialog.floating = true;
                            })),
                        )
                        .child(
                            chip("options-open", t("table.prop.options"), dialog.options_open)
                                .on_click(props(cx, |dialog| {
                                    dialog.options_open = !dialog.options_open
                                })),
                        ),
                );
            if dialog.position_open {
                body = body.child(position_section(dialog, cx));
            }
            if dialog.options_open {
                body = body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_2()
                        .rounded_sm()
                        .border_1()
                        .border_color(theme::border())
                        .child(menu_caption(t("table.prop.default_margins")))
                        .child(
                            line()
                                .child(label("table.prop.top"))
                                .child(number_box(dialog, Field::MarginTop, mm_unit, cx))
                                .child(label("table.prop.left_margin"))
                                .child(number_box(dialog, Field::MarginLeft, mm_unit, cx)),
                        )
                        .child(
                            line()
                                .child(label("table.prop.bottom"))
                                .child(number_box(dialog, Field::MarginBottom, mm_unit, cx))
                                .child(label("table.prop.right_margin"))
                                .child(number_box(dialog, Field::MarginRight, mm_unit, cx)),
                        )
                        .child(
                            line()
                                .child(
                                    checkbox(
                                        "spacing-on",
                                        t("table.prop.spacing"),
                                        dialog.spacing_on,
                                    )
                                    .on_click(props(cx, |dialog| {
                                        dialog.spacing_on = !dialog.spacing_on
                                    })),
                                )
                                .child(number_box(dialog, Field::Spacing, mm_unit, cx)),
                        )
                        .child(
                            checkbox("autofit-on", t("table.prop.autofit"), dialog.autofit)
                                .on_click(props(cx, |dialog| dialog.autofit = !dialog.autofit)),
                        ),
                );
            }
            body
        }
        PropertiesTab::Row => div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(theme::text_dim()).child(format!(
                "{} {}",
                t("table.tab.row"),
                dialog.original.cell.row + 1
            )))
            .child(
                line()
                    .child(
                        checkbox(
                            "row-height-on",
                            t("table.prop.height"),
                            dialog.row_height_on,
                        )
                        .on_click(props(cx, |dialog| {
                            dialog.row_height_on = !dialog.row_height_on
                        })),
                    )
                    .child(number_box(dialog, Field::RowHeight, mm_unit, cx))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("table.prop.height_rule")),
                    )
                    .child(
                        chip("rule-at-least", t("table.prop.at_least"), !dialog.row_exact)
                            .on_click(props(cx, |dialog| dialog.row_exact = false)),
                    )
                    .child(
                        chip("rule-exact", t("table.prop.exact"), dialog.row_exact)
                            .on_click(props(cx, |dialog| dialog.row_exact = true)),
                    ),
            )
            .child(
                checkbox(
                    "allow-break",
                    t("table.prop.allow_break"),
                    dialog.allow_break,
                )
                .on_click(props(cx, |dialog| dialog.allow_break = !dialog.allow_break)),
            )
            .child(
                checkbox(
                    "repeat-header",
                    t("table.prop.repeat_header"),
                    dialog.repeat_header,
                )
                .on_click(props(cx, |dialog| {
                    dialog.repeat_header = !dialog.repeat_header
                })),
            ),
        PropertiesTab::Column => div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().text_color(theme::text_dim()).child(format!(
                "{} {}",
                t("table.tab.column"),
                dialog.original.cell.column + 1
            )))
            .child(
                line()
                    .child(
                        checkbox(
                            "column-width-on",
                            t("table.prop.width"),
                            dialog.column_width_on,
                        )
                        .on_click(props(cx, |dialog| {
                            dialog.column_width_on = !dialog.column_width_on
                        })),
                    )
                    .child(number_box(dialog, Field::ColumnWidth, mm_unit, cx)),
            ),
        PropertiesTab::Cell => {
            let mut body = div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    line()
                        .child(
                            checkbox("cell-width-on", t("table.prop.width"), dialog.cell_width_on)
                                .on_click(props(cx, |dialog| {
                                    dialog.cell_width_on = !dialog.cell_width_on
                                })),
                        )
                        .child(number_box(dialog, Field::CellWidth, mm_unit, cx)),
                )
                .child(
                    line()
                        .child(label("table.prop.vertical"))
                        .children(
                            [
                                ("top", "table.prop.top"),
                                ("center", "table.prop.center"),
                                ("bottom", "table.prop.bottom"),
                            ]
                            .into_iter()
                            .map(|(value, key)| {
                                chip(
                                    SharedString::from(format!("cell-valign-{value}")),
                                    t(key),
                                    dialog.vertical_align == value,
                                )
                                .on_click(props(cx, move |dialog| {
                                    dialog.vertical_align = value.to_string()
                                }))
                            }),
                        )
                        .child(
                            chip(
                                "cell-options-open",
                                t("table.prop.options"),
                                dialog.cell_options_open,
                            )
                            .on_click(props(cx, |dialog| {
                                dialog.cell_options_open = !dialog.cell_options_open
                            })),
                        ),
                );
            if dialog.cell_options_open {
                body = body.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_2()
                        .rounded_sm()
                        .border_1()
                        .border_color(theme::border())
                        .child(
                            checkbox(
                                "cell-margins-same",
                                t("table.prop.same_as_table"),
                                dialog.cell_margins_same,
                            )
                            .on_click(props(cx, |dialog| {
                                dialog.cell_margins_same = !dialog.cell_margins_same
                            })),
                        )
                        .child(
                            line()
                                .child(label("table.prop.top"))
                                .child(number_box(dialog, Field::CellMarginTop, mm_unit, cx))
                                .child(label("table.prop.left_margin"))
                                .child(number_box(dialog, Field::CellMarginLeft, mm_unit, cx)),
                        )
                        .child(
                            line()
                                .child(label("table.prop.bottom"))
                                .child(number_box(dialog, Field::CellMarginBottom, mm_unit, cx))
                                .child(label("table.prop.right_margin"))
                                .child(number_box(dialog, Field::CellMarginRight, mm_unit, cx)),
                        ),
                );
            }
            body
        }
    };
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().flex().gap_1().children(tabs))
        .child(div().min_h(px(200.0)).child(body))
        .into_any_element()
}

/// 表の位置.
fn position_section(dialog: &TablePropertiesDialog, cx: &mut Context<DocumentView>) -> AnyElement {
    let mm_unit = "mm";
    let horizontal = [
        ("left", "table.prop.left"),
        ("center", "table.prop.center"),
        ("right", "table.prop.right"),
        ("inside", "table.prop.inside"),
        ("outside", "table.prop.outside"),
    ];
    let vertical = [
        ("top", "table.prop.top"),
        ("center", "table.prop.center"),
        ("bottom", "table.prop.bottom"),
    ];
    let anchors_x = [
        ("margin", "table.prop.anchor_margin"),
        ("page", "table.prop.anchor_page"),
        ("text", "table.prop.anchor_column"),
    ];
    let anchors_y = [
        ("margin", "table.prop.anchor_margin"),
        ("page", "table.prop.anchor_page"),
        ("text", "table.prop.anchor_paragraph"),
    ];
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_sm()
        .border_1()
        .border_color(theme::border())
        .child(menu_caption(t("table.prop.horizontal")))
        .child(
            line()
                .children(horizontal.into_iter().map(|(value, key)| {
                    chip(
                        SharedString::from(format!("pos-x-{value}")),
                        t(key),
                        dialog.position_x_align.as_deref() == Some(value),
                    )
                    .on_click(props(cx, move |dialog| {
                        dialog.position_x_align = Some(value.to_string())
                    }))
                }))
                .child(number_box(dialog, Field::PositionHorizontal, mm_unit, cx)),
        )
        .child(
            line()
                .child(label("table.prop.relative_to"))
                .children(anchors_x.into_iter().map(|(value, key)| {
                    chip(
                        SharedString::from(format!("anchor-x-{value}")),
                        t(key),
                        dialog.horizontal_anchor == value,
                    )
                    .on_click(props(cx, move |dialog| {
                        dialog.horizontal_anchor = value.to_string()
                    }))
                })),
        )
        .child(menu_caption(t("table.prop.vertical_position")))
        .child(
            line()
                .children(vertical.into_iter().map(|(value, key)| {
                    chip(
                        SharedString::from(format!("pos-y-{value}")),
                        t(key),
                        dialog.position_y_align.as_deref() == Some(value),
                    )
                    .on_click(props(cx, move |dialog| {
                        dialog.position_y_align = Some(value.to_string())
                    }))
                }))
                .child(number_box(dialog, Field::PositionVertical, mm_unit, cx)),
        )
        .child(
            line()
                .child(label("table.prop.relative_to"))
                .children(anchors_y.into_iter().map(|(value, key)| {
                    chip(
                        SharedString::from(format!("anchor-y-{value}")),
                        t(key),
                        dialog.vertical_anchor == value,
                    )
                    .on_click(props(cx, move |dialog| {
                        dialog.vertical_anchor = value.to_string()
                    }))
                })),
        )
        .child(menu_caption(t("table.prop.distance")))
        .child(
            line()
                .child(label("table.prop.top"))
                .child(number_box(dialog, Field::DistanceTop, mm_unit, cx))
                .child(label("table.prop.left_margin"))
                .child(number_box(dialog, Field::DistanceLeft, mm_unit, cx)),
        )
        .child(
            line()
                .child(label("table.prop.bottom"))
                .child(number_box(dialog, Field::DistanceBottom, mm_unit, cx))
                .child(label("table.prop.right_margin"))
                .child(number_box(dialog, Field::DistanceRight, mm_unit, cx)),
        )
        .child(
            line()
                .child(
                    checkbox(
                        "move-with-text",
                        t("table.prop.move_with_text"),
                        dialog.vertical_anchor == "text",
                    )
                    .on_click(props(cx, |dialog| {
                        dialog.vertical_anchor = if dialog.vertical_anchor == "text" {
                            "margin".to_string()
                        } else {
                            "text".to_string()
                        }
                    })),
                )
                .child(
                    checkbox(
                        "allow-overlap",
                        t("table.prop.allow_overlap"),
                        dialog.allow_overlap,
                    )
                    .on_click(props(cx, |dialog| {
                        dialog.allow_overlap = !dialog.allow_overlap
                    })),
                ),
        )
        .into_any_element()
}

/// A click that edits 罫線と網かけ.
fn borders(
    cx: &mut Context<DocumentView>,
    change: impl Fn(&mut BordersDialog) + 'static,
) -> impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static {
    edit(cx, move |dialog| {
        if let TableDialog::Borders(borders) = dialog {
            change(borders);
        }
    })
}

fn borders_body(dialog: &BordersDialog, cx: &mut Context<DocumentView>) -> AnyElement {
    let edge_names = [
        "table.border.top",
        "table.border.bottom",
        "table.border.left",
        "table.border.right",
        "table.border.inside_h",
        "table.border.inside_v",
    ];
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            line()
                .child(label("table.borders_kind"))
                .child(
                    chip(
                        "preset-none",
                        t("table.border.none"),
                        dialog.edges == [false; 6],
                    )
                    .on_click(borders(cx, |d| d.preset(BorderSet::None))),
                )
                .child(
                    chip(
                        "preset-box",
                        t("table.border.outside"),
                        dialog.edges == [true, true, true, true, false, false],
                    )
                    .on_click(borders(cx, |d| d.preset(BorderSet::Outside))),
                )
                .child(
                    chip(
                        "preset-all",
                        t("table.border.all"),
                        dialog.edges == [true; 6],
                    )
                    .on_click(borders(cx, |d| d.preset(BorderSet::All))),
                ),
        )
        .child(line().child(label("table.borders_edges")).children(
            edge_names.into_iter().enumerate().map(|(index, key)| {
                chip(("edge", index), t(key), dialog.edges[index])
                    .on_click(borders(cx, move |d| d.edges[index] = !d.edges[index]))
            }),
        ))
        .child(
            line()
                .child(label("table.pen_style"))
                .children(Pen::STYLES.iter().enumerate().map(|(index, style)| {
                    let style = *style;
                    chip(
                        ("border-style", index),
                        t(line_key(style)),
                        dialog.pen.style == style,
                    )
                    .on_click(borders(cx, move |d| d.pen.style = style.to_string()))
                })),
        )
        .child(
            line()
                .child(label("table.pen_width"))
                .children(Pen::WIDTHS.iter().enumerate().map(|(index, width)| {
                    let width = *width;
                    chip(
                        ("border-width", index),
                        format!("{width} pt"),
                        (dialog.pen.width - width).abs() < 0.01,
                    )
                    .on_click(borders(cx, move |d| d.pen.width = width))
                })),
        )
        .child(
            line()
                .child(label("table.pen_color"))
                .children(PEN_COLORS.iter().enumerate().map(|(index, color)| {
                    let color = *color;
                    let shown = if color == "auto" { "000000" } else { color };
                    swatch_box(("border-color", index), shown, dialog.pen.color == color)
                        .on_click(borders(cx, move |d| d.pen.color = color.to_string()))
                })),
        )
        .child(
            line()
                .child(label("table.shading"))
                .child(
                    chip("fill-keep", t("table.fill_keep"), dialog.fill.is_none())
                        .on_click(borders(cx, |d| d.fill = None)),
                )
                .child(
                    chip("fill-none", t("table.no_color"), dialog.fill == Some(None))
                        .on_click(borders(cx, |d| d.fill = Some(None))),
                )
                .children(SHADES.iter().enumerate().map(|(index, color)| {
                    let color = *color;
                    swatch_box(
                        ("fill", index),
                        color,
                        dialog.fill.as_ref().and_then(|fill| fill.as_deref()) == Some(color),
                    )
                    .on_click(borders(cx, move |d| d.fill = Some(Some(color.to_string()))))
                })),
        )
        .child(
            line()
                .child(label("table.apply_to"))
                .child(
                    chip(
                        "target-table",
                        t("table.apply_table"),
                        dialog.target == BorderTarget::Table,
                    )
                    .on_click(borders(cx, |d| d.target = BorderTarget::Table)),
                )
                .child(
                    chip(
                        "target-cell",
                        t("table.apply_cell"),
                        dialog.target == BorderTarget::Cell,
                    )
                    .on_click(borders(cx, |d| d.target = BorderTarget::Cell)),
                ),
        )
        .into_any_element()
}

fn cells_body(delete: bool, shift: CellShift, cx: &mut Context<DocumentView>) -> AnyElement {
    let choices = if delete {
        [
            (CellShift::Horizontal, "table.shift.left"),
            (CellShift::Vertical, "table.shift.up"),
            (CellShift::EntireRow, "table.shift.delete_row"),
            (CellShift::EntireColumn, "table.shift.delete_column"),
        ]
    } else {
        [
            (CellShift::Horizontal, "table.shift.right"),
            (CellShift::Vertical, "table.shift.down"),
            (CellShift::EntireRow, "table.shift.insert_row"),
            (CellShift::EntireColumn, "table.shift.insert_column"),
        ]
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(
            choices
                .into_iter()
                .enumerate()
                .map(|(index, (choice, key))| {
                    chip(("shift", index), t(key), shift == choice).on_click(edit(
                        cx,
                        move |dialog| {
                            if let TableDialog::Cells { shift, .. } = dialog {
                                *shift = choice;
                            }
                        },
                    ))
                }),
        )
        .into_any_element()
}

/// A plain box of the small dialogs.
fn plain_box(
    id: &'static str,
    value: &str,
    focused: bool,
    focus: DialogBox,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .min_w(px(54.0))
        .px_1()
        .rounded_sm()
        .border_1()
        .border_color(if focused {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(theme::window())
        .cursor_pointer()
        .child(value.to_string())
        .on_click(edit(cx, move |dialog| match dialog {
            TableDialog::Split { focus: current, .. }
            | TableDialog::ToTable { focus: current, .. } => *current = focus,
            _ => {}
        }))
        .into_any_element()
}

fn split_body(
    columns: &str,
    rows: &str,
    focus: DialogBox,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(line().child(label("table.columns")).child(plain_box(
            "split-columns",
            columns,
            focus == DialogBox::Columns,
            DialogBox::Columns,
            cx,
        )))
        .child(line().child(label("table.rows")).child(plain_box(
            "split-rows",
            rows,
            focus == DialogBox::Rows,
            DialogBox::Rows,
            cx,
        )))
        .into_any_element()
}

fn separator_line(
    current: Separator,
    other: &str,
    paragraph_key: &'static str,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let choices = [
        (Separator::Paragraph, paragraph_key),
        (Separator::Tab, "table.separator.tab"),
        (Separator::Comma, "table.separator.comma"),
        (Separator::Other, "table.separator.other"),
    ];
    line()
        .child(label("table.separator"))
        .children(
            choices
                .into_iter()
                .enumerate()
                .map(|(index, (choice, key))| {
                    chip(("separator", index), t(key), current == choice).on_click(edit(
                        cx,
                        move |dialog| match dialog {
                            TableDialog::ToTable {
                                separator, focus, ..
                            } => {
                                *separator = choice;
                                if choice == Separator::Other {
                                    *focus = DialogBox::Other;
                                }
                            }
                            TableDialog::ToText { separator, .. } => *separator = choice,
                            _ => {}
                        },
                    ))
                }),
        )
        .child(
            div()
                .w(px(28.0))
                .px_1()
                .rounded_sm()
                .border_1()
                .border_color(if current == Separator::Other {
                    theme::accent()
                } else {
                    theme::border()
                })
                .child(other.to_string()),
        )
        .into_any_element()
}

fn to_table_body(
    separator: Separator,
    other: &str,
    columns: &str,
    focus: DialogBox,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(line().child(label("table.columns")).child(plain_box(
            "to-table-columns",
            columns,
            focus == DialogBox::Columns,
            DialogBox::Columns,
            cx,
        )))
        .child(separator_line(
            separator,
            other,
            "table.separator.paragraph",
            cx,
        ))
        .into_any_element()
}

fn to_text_body(separator: Separator, other: &str, cx: &mut Context<DocumentView>) -> AnyElement {
    separator_line(separator, other, "table.separator.paragraph_mark", cx)
}

fn sort_body(
    keys: &[SortSlot; 3],
    header: bool,
    headings: &[String],
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let key_names = ["table.sort.first", "table.sort.second", "table.sort.third"];
    let edit_key =
        |cx: &mut Context<DocumentView>, slot: usize, change: Box<dyn Fn(&mut SortSlot)>| {
            edit(cx, move |dialog| {
                if let TableDialog::Sort { keys, .. } = dialog {
                    change(&mut keys[slot]);
                    // The first key is always there.
                    if keys[0].is_none() {
                        keys[0] = Some((0, HeadlessTableSortKind::Text, false));
                    }
                }
            })
        };
    let mut body = div().flex().flex_col().gap_2();
    for (slot, name) in key_names.into_iter().enumerate() {
        let key = keys[slot];
        let mut columns = line().child(label(name));
        if slot > 0 {
            columns =
                columns.child(
                    chip(("sort-none", slot), t("table.sort.none"), key.is_none())
                        .on_click(edit_key(cx, slot, Box::new(|key| *key = None))),
                );
        }
        for (column, heading) in headings.iter().enumerate() {
            let on = key.is_some_and(|key| key.0 == column);
            columns = columns.child(
                chip(
                    SharedString::from(format!("sort-{slot}-{column}")),
                    heading.clone(),
                    on,
                )
                .on_click(edit_key(
                    cx,
                    slot,
                    Box::new(move |key| {
                        let (kind, descending) =
                            key.map_or((HeadlessTableSortKind::Text, false), |key| (key.1, key.2));
                        *key = Some((column, kind, descending));
                    }),
                )),
            );
        }
        body = body.child(columns);
        if let Some((_, kind, descending)) = key {
            let kinds = [
                (HeadlessTableSortKind::Text, "table.sort.text"),
                (HeadlessTableSortKind::Number, "table.sort.number"),
                (HeadlessTableSortKind::Date, "table.sort.date"),
            ];
            body =
                body.child(
                    line()
                        .child(div().w(px(132.0)))
                        .children(kinds.into_iter().enumerate().map(
                            |(index, (choice, key_name))| {
                                chip(
                                    SharedString::from(format!("sort-kind-{slot}-{index}")),
                                    t(key_name),
                                    kind == choice,
                                )
                                .on_click(edit_key(
                                    cx,
                                    slot,
                                    Box::new(move |key| {
                                        if let Some(key) = key.as_mut() {
                                            key.1 = choice;
                                        }
                                    }),
                                ))
                            },
                        ))
                        .child(
                            chip(
                                ("sort-ascending", slot),
                                t("table.sort.ascending"),
                                !descending,
                            )
                            .on_click(edit_key(
                                cx,
                                slot,
                                Box::new(|key| {
                                    if let Some(key) = key.as_mut() {
                                        key.2 = false;
                                    }
                                }),
                            )),
                        )
                        .child(
                            chip(
                                ("sort-descending", slot),
                                t("table.sort.descending"),
                                descending,
                            )
                            .on_click(edit_key(
                                cx,
                                slot,
                                Box::new(|key| {
                                    if let Some(key) = key.as_mut() {
                                        key.2 = true;
                                    }
                                }),
                            )),
                        ),
                );
        }
    }
    body.child(
        line()
            .child(label("table.sort.header"))
            .child(
                chip("sort-header-yes", t("table.sort.header_yes"), header).on_click(edit(
                    cx,
                    |dialog| {
                        if let TableDialog::Sort { header, .. } = dialog {
                            *header = true;
                        }
                    },
                )),
            )
            .child(
                chip("sort-header-no", t("table.sort.header_no"), !header).on_click(edit(
                    cx,
                    |dialog| {
                        if let TableDialog::Sort { header, .. } = dialog {
                            *header = false;
                        }
                    },
                )),
            ),
    )
    .into_any_element()
}

fn formula_body(formula: &str, format: &str, cx: &mut Context<DocumentView>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            line().child(label("table.formula")).child(
                div()
                    .min_w(px(200.0))
                    .px_1()
                    .rounded_sm()
                    .border_1()
                    .border_color(theme::accent())
                    .bg(theme::window())
                    .child(formula.to_string()),
            ),
        )
        .child(line().child(label("table.formula_format")).children(
            NUMBER_FORMATS.iter().enumerate().map(|(index, picture)| {
                let picture = *picture;
                chip(
                    ("number-format", index),
                    if picture.is_empty() {
                        t("table.sort.none").to_string()
                    } else {
                        picture.to_string()
                    },
                    format == picture,
                )
                .on_click(edit(cx, move |dialog| {
                    if let TableDialog::Formula { format, .. } = dialog {
                        *format = picture;
                    }
                }))
            }),
        ))
        .child(line().child(label("table.formula_paste")).children(
            FUNCTIONS.iter().enumerate().map(|(index, function)| {
                let function = *function;
                chip(("paste-function", index), function, false).on_click(edit(cx, move |dialog| {
                    if let TableDialog::Formula { formula, .. } = dialog {
                        if formula.trim().is_empty() || formula.trim() == "=" {
                            *formula = "=".to_string();
                        }
                        formula.push_str(function);
                        formula.push_str("()");
                    }
                }))
            }),
        ))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Separator, TableAction, TableDialog};
    use crate::ribbon::Action;
    use crate::RibbonTab;
    use jade_office_model::word::table::{
        LookOption, PropertiesTab, TableHit, TablePart, WordTables,
    };
    use jade_office_model::word::WordSession;

    /// A blank document in a window, with a 3 × 3 table and the caret in it.
    fn with_table(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::Japanese,
        );
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::untitled_view(&bytes, "表.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::InsertTable(3, 3), cx);
            assert!(view.format_state().table.is_some());
        });
        (view, cx)
    }

    /// The contextual tabs appear in a table, and their controls change it:
    /// a style option, a gallery style, a selection, a dialog's OK.
    #[gpui::test]
    fn the_table_tabs_drive_the_caret_table(cx: &mut gpui::TestAppContext) {
        let (view, cx) = with_table(cx);
        view.update(cx, |view, cx| {
            assert!(view
                .ribbon_tabs()
                .iter()
                .any(|(tab, _)| *tab == RibbonTab::TableDesign));
            view.ribbon_tab = RibbonTab::TableDesign;
            view.run_ribbon(
                Action::Tables(TableAction::Style(Some("GridTable4-Accent1".to_string()))),
                cx,
            );
            view.run_ribbon(
                Action::Tables(TableAction::Look(LookOption::TotalRow, true)),
                cx,
            );
            assert_eq!(
                view.document.table_style_id().as_deref(),
                Some("GridTable4-Accent1")
            );
            assert!(view.document.table_look().unwrap().total_row);
            view.ribbon_tab = RibbonTab::TableLayout;
            view.run_ribbon(Action::Tables(TableAction::Select(TablePart::Column)), cx);
            let span = view.document.cell_span().unwrap();
            assert_eq!(span.rows, (0, 2));
            cx.notify();
        });
        // Drawn with the table tabs showing.
        cx.run_until_parked();
        view.update(cx, |view, cx| {
            view.run_ribbon(
                Action::Tables(TableAction::Properties(PropertiesTab::Row)),
                cx,
            );
            let Some(TableDialog::Properties(dialog)) = view.tables.dialog.as_mut() else {
                panic!("the properties dialog is open");
            };
            dialog.row_height_on = true;
            dialog.focus = Some(jade_office_model::word::table::Field::RowHeight);
            dialog.text.insert(
                jade_office_model::word::table::Field::RowHeight,
                String::new(),
            );
            cx.notify();
        });
        cx.simulate_keystrokes("1 5");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, _| {
            assert!(view.tables.dialog.is_none());
            let properties = view.document.table_properties().unwrap();
            assert!(
                (properties.row_height.unwrap() - 15.0 / jade_office_model::word::table::MM_PER_PT)
                    .abs()
                    < 0.5
            );
        });
        // Out of the table, the tabs go away.
        view.update(cx, |view, cx| {
            let outside = (0..view.document.paragraph_count())
                .find(|paragraph| view.document.cell_id_for_paragraph(*paragraph).is_none())
                .expect("a paragraph outside the table");
            view.document.place_caret(outside, 0);
            view.insert_at = (outside, 0);
            view.format = None;
            assert!(!view
                .ribbon_tabs()
                .iter()
                .any(|(tab, _)| *tab == RibbonTab::TableDesign));
            cx.notify();
        });
        cx.run_until_parked();
    }

    /// The mouse: a border dragged resizes, the row selector selects the
    /// row, and the pencil splits a cell.
    #[gpui::test]
    fn the_mouse_resizes_selects_and_draws(cx: &mut gpui::TestAppContext) {
        let (view, cx) = with_table(cx);
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            let table_id = view
                .document
                .cell_layout(view.document.cell_id_for_paragraph(paragraph).unwrap())
                .unwrap()
                .table_id;
            let geometry = view.document.table_geometry(table_id).unwrap();
            let (top, bottom) = geometry.rows[0];
            let y = (top + bottom) / 2.0;
            let before = geometry.widths_pt();
            view.tables.drag = Some((
                super::TableDrag::Column {
                    table_id,
                    boundary: 1,
                },
                0,
                (geometry.columns[1], y),
                (geometry.columns[1] + 30.0, y),
            ));
            view.table_mouse_up(cx);
            let table_id = view
                .document
                .cell_layout(
                    view.document
                        .cell_id_for_paragraph(view.insert_at.0)
                        .unwrap(),
                )
                .unwrap()
                .table_id;
            let after = view.document.table_geometry(table_id).unwrap().widths_pt();
            assert!(
                (after[0] - before[0] - 22.5).abs() < 1.0,
                "{before:?} {after:?}"
            );

            let geometry = view.document.table_geometry(table_id).unwrap();
            let (top, bottom) = geometry.rows[1];
            let hit = view
                .document
                .table_hit(geometry.left() - 8.0, (top + bottom) / 2.0)
                .unwrap();
            assert!(matches!(hit, TableHit::RowSelector { row: 1, .. }));
            view.document.click_table_hit(hit).unwrap();
            assert_eq!(view.document.cell_span().unwrap().rows, (1, 1));

            view.run_ribbon(
                Action::Tables(TableAction::Tool(
                    jade_office_model::word::table::DrawTool::Pencil,
                )),
                cx,
            );
            assert_eq!(
                super::cursor_style(&view.tables),
                gpui::CursorStyle::Crosshair
            );
            let x = (geometry.columns[0] + geometry.columns[1]) / 2.0;
            let (top, bottom) = geometry.rows[2];
            view.tables.drag = Some((
                super::TableDrag::Stroke,
                0,
                (x, top + 1.0),
                (x, bottom - 1.0),
            ));
            view.table_mouse_up(cx);
            assert_eq!(
                view.document.word_document().table_dimensions(0),
                Some((3, 4))
            );
        });
    }

    /// The small dialogs: cells inserted with a shift, text made a table and
    /// back, a sort and a formula.
    #[gpui::test]
    fn the_small_dialogs_do_what_they_say(cx: &mut gpui::TestAppContext) {
        let (view, cx) = with_table(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Tables(TableAction::InsertCells), cx);
            view.apply_table_dialog(cx);
            assert_eq!(
                view.document.word_document().table_dimensions(0),
                Some((4, 3))
            );
            view.run_ribbon(Action::Tables(TableAction::Formula), cx);
            assert!(matches!(
                view.tables.dialog,
                Some(TableDialog::Formula { .. })
            ));
            view.tables.dialog = None;
            view.run_ribbon(Action::Tables(TableAction::ToText), cx);
            if let Some(TableDialog::ToText { separator, .. }) = view.tables.dialog.as_mut() {
                *separator = Separator::Comma;
            }
            view.apply_table_dialog(cx);
            assert!(view.document.word_document().table_by_order(0).is_none());
            assert!(view.table_status.is_none(), "{:?}", view.table_status);
        });
    }
}

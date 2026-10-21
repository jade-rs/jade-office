//! 数式: equations in the native shell. 挿入 > 数式 puts a new equation (or
//! a built-in one from its list) at the caret; clicking an equation edits it
//! in place, with the 数式 tab — its 数式 gallery, 2 次元形式 and 行形式
//! with Unicode or LaTeX, 標準テキスト, the 記号と特殊文字 gallery and the
//! 構造 lists — while the caret is in it. The editing itself is
//! `jade-office-model`'s [`EquationEditor`]; this module routes keys, typing
//! and clicks to it and draws its caret over the page.

use gpui::{div, prelude::*, px, rgb, AnyElement, Context, KeyDownEvent, SharedString};
use jade_office_model::word::equation::{
    built_in_equation, insert_new_equation, zone_at, zone_for, EquationEditor, EquationFormat,
    EquationInsert, LinearFormat, StructureGroup, BUILT_IN_EQUATIONS, SYMBOL_CATEGORIES,
};
use jade_office_model::word::WordSession;

use crate::ribbon::{
    dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup,
};
use crate::tooltip::Tooltip;
use crate::{i18n, theme, DocumentView, RibbonTab};

/// The equation being edited, and the tab's state.
#[derive(Default)]
pub struct EquationUi {
    pub editor: Option<EquationEditor>,
    /// The 記号と特殊文字 category the tab's gallery shows.
    pub symbols: usize,
}

/// The lists the 数式 controls drop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquationPopup {
    BuiltIns,
    Structure(StructureGroup),
    Symbols,
}

/// What the 数式 controls do.
#[derive(Clone, Debug, PartialEq)]
pub enum EquationAction {
    /// 挿入 > 数式: a new equation at the caret.
    InsertNew,
    /// A built-in equation, at the caret or into the equation being edited.
    BuiltIn(usize),
    /// A structure or symbol into the equation being edited.
    Insert(EquationInsert),
    /// 2 次元形式.
    Professional,
    /// 行形式.
    Linear,
    /// Unicode or LaTeX.
    Format(LinearFormat),
    /// 標準テキスト.
    NormalText,
    SymbolCategory(usize),
    /// 行を上に挿入 / 行を下に挿入 (`true` below).
    InsertRow(bool),
    /// 列を左に挿入 / 列を右に挿入 (`true` right).
    InsertColumn(bool),
    DeleteRow,
    DeleteColumn,
    /// 列の配置: left, center or right.
    ColumnJustification(&'static str),
    /// 手動改行を挿入.
    LineBreak,
    /// この文字で揃える.
    AlignPoint,
    /// Leaves the equation.
    Exit,
}

fn action(action: EquationAction) -> Action {
    Action::Equation(action)
}

/// Runs a 数式 control.
pub fn run(view: &mut DocumentView, act: EquationAction, cx: &mut Context<DocumentView>) {
    view.ribbon.popup = None;
    match act {
        EquationAction::InsertNew => insert(view, None, cx),
        EquationAction::BuiltIn(index) => {
            if let Some(editor) = view.equation.editor.as_mut() {
                editor.insert(&EquationInsert::BuiltIn(index));
                commit(view, cx);
            } else {
                let equation = built_in_equation(index);
                insert(view, equation.as_ref(), cx);
            }
        }
        EquationAction::Insert(item) => {
            if let Some(editor) = view.equation.editor.as_mut() {
                editor.insert(&item);
                commit(view, cx);
            }
        }
        EquationAction::Professional => {
            if let Some(editor) = view.equation.editor.as_mut() {
                editor.to_professional_form();
                commit(view, cx);
            }
        }
        EquationAction::Linear => {
            if let Some(editor) = view.equation.editor.as_mut() {
                editor.to_linear_form();
                commit(view, cx);
            }
        }
        EquationAction::Format(format) => {
            if let Some(editor) = view.equation.editor.as_mut() {
                editor.format = format;
            }
        }
        EquationAction::NormalText => {
            if let Some(editor) = view.equation.editor.as_mut() {
                editor.normal_text = !editor.normal_text;
            }
        }
        EquationAction::SymbolCategory(index) => view.equation.symbols = index,
        EquationAction::InsertRow(_)
        | EquationAction::InsertColumn(_)
        | EquationAction::DeleteRow
        | EquationAction::DeleteColumn
        | EquationAction::ColumnJustification(_)
        | EquationAction::LineBreak
        | EquationAction::AlignPoint => {
            let done = view
                .equation
                .editor
                .as_mut()
                .is_some_and(|editor| match &act {
                    EquationAction::InsertRow(below) => editor.insert_row(*below),
                    EquationAction::InsertColumn(after) => editor.insert_column(*after),
                    EquationAction::DeleteRow => editor.delete_row(),
                    EquationAction::DeleteColumn => editor.delete_column(),
                    EquationAction::ColumnJustification(jc) => editor.set_column_justification(jc),
                    EquationAction::LineBreak => editor.insert_line_break(),
                    _ => editor.set_alignment_point(),
                });
            if done {
                commit(view, cx);
            }
        }
        EquationAction::Exit => close(view),
    }
    cx.notify();
}

/// Inserts a new equation (empty, or `content`) at the caret and starts
/// editing it.
fn insert(
    view: &mut DocumentView,
    content: Option<&jade_docx_editor::document::MathElement>,
    cx: &mut Context<DocumentView>,
) {
    close(view);
    // The caret, or the end of the document when it is past it (a new
    // document's caret before the first click).
    let count = view.document.paragraph_count();
    let at = if view.insert_at.0 < count {
        view.insert_at
    } else {
        let last = count.saturating_sub(1);
        let end = view.document.paragraph_text(last).map_or(0, |t| t.len());
        (last, end)
    };
    match insert_new_equation(&mut view.document, at, content) {
        Ok(target) => {
            view.dirty = true;
            view.rebuild_all_images();
            let zones = WordSession::math_zones(&view.document);
            if let Some(zone) = zone_for(&zones, &target) {
                view.equation.editor = EquationEditor::open(&view.document, zone);
                view.ribbon_tab = RibbonTab::Equation;
            }
        }
        Err(error) => view.table_status = Some(SharedString::from(error)),
    }
    cx.notify();
}

/// ホーム's font size, colour and bold, while an equation is edited: they
/// format the equation, as Word applies them to it.
pub fn home_format(action: &Action) -> Option<EquationFormat> {
    use jade_office_model::word::CharacterFormat;
    Some(match action {
        Action::FontSize(points) => EquationFormat {
            size: Some(*points),
            ..EquationFormat::default()
        },
        Action::FontColor(color) => EquationFormat {
            color: Some(color.clone()),
            ..EquationFormat::default()
        },
        Action::Toggle(CharacterFormat::Bold) => EquationFormat {
            bold: Some(true),
            ..EquationFormat::default()
        },
        _ => return None,
    })
}

/// Applies ホーム's formatting to the equation being edited.
pub fn format(view: &mut DocumentView, format: EquationFormat, cx: &mut Context<DocumentView>) {
    view.ribbon.popup = None;
    if let Some(editor) = view.equation.editor.as_mut() {
        editor.format_runs(format);
        commit(view, cx);
    }
}

/// Writes the edited equation back and repaints.
fn commit(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(editor) = view.equation.editor.as_ref() else {
        return;
    };
    match editor.commit(&mut view.document) {
        Ok(()) => {
            view.dirty = true;
            view.rebuild_all_images();
        }
        Err(error) => view.table_status = Some(SharedString::from(error)),
    }
    cx.notify();
}

/// Leaves the equation being edited.
pub fn close(view: &mut DocumentView) {
    if view.equation.editor.take().is_some() && view.ribbon_tab == RibbonTab::Equation {
        view.ribbon_tab = RibbonTab::Home;
    }
}

/// After undo or redo: the equation is read again from the document, the
/// caret kept where it still exists.
pub fn after_history(view: &mut DocumentView) {
    let Some(editor) = view.equation.editor.as_ref() else {
        return;
    };
    let zones = WordSession::math_zones(&view.document);
    let refreshed = editor
        .zone(&zones)
        .and_then(|zone| EquationEditor::open(&view.document, zone));
    view.equation.editor = refreshed;
}

/// Typed text, from the platform's input handler.
pub fn type_text(view: &mut DocumentView, text: &str, cx: &mut Context<DocumentView>) {
    let Some(editor) = view.equation.editor.as_mut() else {
        return;
    };
    for ch in text.chars() {
        editor.type_text(&ch.to_string());
    }
    commit(view, cx);
}

/// The keys the equation editor takes while it is open. Returns whether
/// the key was taken.
pub fn handle_key(
    view: &mut DocumentView,
    event: &KeyDownEvent,
    cx: &mut Context<DocumentView>,
) -> bool {
    if view.equation.editor.is_none() {
        return false;
    }
    let modifiers = event.keystroke.modifiers;
    // Shortcuts (⌘Z, ⌘S …) are the menus'.
    if modifiers.platform || modifiers.control {
        return false;
    }
    let key = event.keystroke.key.as_str();
    let zones = WordSession::math_zones(&view.document);
    let Some(editor) = view.equation.editor.as_mut() else {
        return false;
    };
    let zone = editor.zone(&zones).cloned();
    let edited = match key {
        "left" => {
            editor.move_left();
            false
        }
        "right" => {
            editor.move_right();
            false
        }
        "up" | "down" => {
            if let Some(zone) = zone.as_ref() {
                editor.move_vertically(zone, key == "down");
            }
            false
        }
        "home" => {
            editor.move_home();
            false
        }
        "end" => {
            editor.move_end();
            false
        }
        "tab" => {
            editor.next_slot();
            false
        }
        "backspace" => {
            editor.backspace();
            true
        }
        "delete" => {
            editor.delete_forward();
            true
        }
        // ⇧Enter: a manual line break, as Word's 手動改行を挿入.
        "enter" if modifiers.shift => editor.insert_line_break(),
        "escape" | "enter" => {
            close(view);
            cx.notify();
            return true;
        }
        _ => return false,
    };
    if edited {
        commit(view, cx);
    } else {
        cx.notify();
    }
    true
}

/// A click on a page: into an equation edits it there; anywhere else
/// leaves the one being edited. Returns whether the click was taken.
pub fn click(
    view: &mut DocumentView,
    page_index: usize,
    x: f64,
    y: f64,
    cx: &mut Context<DocumentView>,
) -> bool {
    let zones = WordSession::math_zones(&view.document);
    let Some(zone) = zone_at(&zones, page_index + 1, x, y).cloned() else {
        close(view);
        return false;
    };
    let same = view.equation.editor.as_ref().is_some_and(|editor| {
        editor
            .zone(&zones)
            .is_some_and(|z| z.paragraph == zone.paragraph && z.offset == zone.offset)
    });
    if !same {
        view.equation.editor = EquationEditor::open(&view.document, &zone);
    }
    if let Some(editor) = view.equation.editor.as_mut() {
        editor.click(&zone, x, y);
        view.ribbon_tab = RibbonTab::Equation;
        view.selected_object = None;
        cx.notify();
        return true;
    }
    false
}

/// The equation's frame, its empty arguments and the caret, over a page.
pub fn overlay(view: &DocumentView, page_index: usize) -> Option<AnyElement> {
    let editor = view.equation.editor.as_ref()?;
    let zones = WordSession::math_zones(&view.document);
    let zone = editor.zone(&zones)?;
    if zone.page != page_index + 1 {
        return None;
    }
    let zoom = view.zoom;
    let at = |value: f64| px(value as f32 * zoom);
    let (frame, placeholders) = editor.frames(zone);
    let caret = editor.caret_geometry(zone);
    let mut children: Vec<AnyElement> = Vec::new();
    // Word frames the equation it is editing.
    children.push(
        div()
            .absolute()
            .left(at(frame.x))
            .top(at(frame.y))
            .w(at(frame.width))
            .h(at(frame.height))
            .border_1()
            .border_color(rgb(0x9aa4b2))
            .bg(gpui::rgba(0xe8ecf233))
            .into_any_element(),
    );
    for slot in placeholders {
        children.push(
            div()
                .absolute()
                .left(at(slot.x))
                .top(at(slot.y))
                .w(at(slot.width))
                .h(at(slot.height))
                .border_1()
                .border_dashed()
                .border_color(rgb(0x7f8a99))
                .into_any_element(),
        );
    }
    if editor.is_empty() {
        children.push(
            div()
                .absolute()
                .left(at(frame.x + 2.0))
                .top(at(frame.y))
                .h(at(frame.height))
                .flex()
                .items_center()
                .text_size(px(12.0 * zoom))
                .text_color(rgb(0x7f8a99))
                .whitespace_nowrap()
                .child(i18n::text("equation.placeholder"))
                .into_any_element(),
        );
    }
    if let Some(caret) = caret {
        children.push(
            div()
                .absolute()
                .left(at(caret.x))
                .top(at(caret.top))
                .w(px((1.5 * zoom).max(1.0)))
                .h(at(caret.height))
                .bg(theme::caret())
                .into_any_element(),
        );
    }
    Some(
        div()
            .absolute()
            .inset_0()
            .children(children)
            .into_any_element(),
    )
}

fn button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
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
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .when(active, |b| b.bg(theme::selected()))
        .cursor_pointer()
        .hover(|b| b.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(on_click, cx))
        .child(label.into())
        .into_any_element()
}

fn menu_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    tooltip: &'static str,
    popup: EquationPopup,
    open: bool,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap_0p5()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .when(open, |b| b.bg(theme::selected()))
        .cursor_pointer()
        .hover(|b| b.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(Action::Popup(Popup::Equation(popup)), cx))
        .child(label.into())
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element()
}

/// The built-in equations' list.
fn built_ins_menu(cx: &mut Context<DocumentView>) -> AnyElement {
    let mut panel = menu_panel("equation-built-ins")
        .child(menu_caption(i18n::text("equation.builtin.caption")));
    for (index, (key, text)) in BUILT_IN_EQUATIONS.iter().enumerate() {
        panel = panel.child(
            div()
                .flex()
                .flex_col()
                .child(menu_row(
                    ("equation-built-in", index),
                    i18n::text(key),
                    false,
                    action(EquationAction::BuiltIn(index)),
                    cx,
                ))
                .child(
                    div()
                        .pl_6()
                        .pb_1()
                        .text_color(theme::text_dim())
                        .child(SharedString::from(*text)),
                ),
        );
    }
    panel.into_any_element()
}

/// 挿入's 数式 group: the button and its list of built-ins.
pub fn insert_group(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let open = view.ribbon.popup == Some(Popup::Equation(EquationPopup::BuiltIns));
    let menu = open.then(|| built_ins_menu(cx));
    group(
        i18n::text("equation.group"),
        vec![row(vec![
            button(
                "insert-equation",
                i18n::text("equation.insert"),
                i18n::text("equation.insert_title"),
                false,
                action(EquationAction::InsertNew),
                cx,
            ),
            dropdown(
                menu_button(
                    "insert-equation-gallery",
                    "",
                    i18n::text("equation.builtin.caption"),
                    EquationPopup::BuiltIns,
                    open,
                    cx,
                ),
                menu,
            ),
        ])],
    )
}

/// The 数式 tab, while an equation is being edited.
pub fn tab_groups(view: &DocumentView, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let Some(editor) = view.equation.editor.as_ref() else {
        return Vec::new();
    };
    let popup = view.ribbon.popup;
    let is_open = |p: EquationPopup| popup == Some(Popup::Equation(p));
    let format = editor.format;
    let normal = editor.normal_text;
    let mut groups = Vec::new();
    // ツール: the built-in equations.
    let built_ins_open = is_open(EquationPopup::BuiltIns);
    let built_ins = built_ins_open.then(|| built_ins_menu(cx));
    groups.push(group(
        i18n::text("equation.group.tools"),
        vec![row(vec![dropdown(
            menu_button(
                "equation-built-ins",
                i18n::text("equation.insert"),
                i18n::text("equation.builtin.caption"),
                EquationPopup::BuiltIns,
                built_ins_open,
                cx,
            ),
            built_ins,
        )])],
    ));
    // 変換.
    groups.push(group(
        i18n::text("equation.group.convert"),
        vec![
            row(vec![
                button(
                    "equation-unicode",
                    "Unicode",
                    i18n::text("equation.unicode_title"),
                    format == LinearFormat::UnicodeMath,
                    action(EquationAction::Format(LinearFormat::UnicodeMath)),
                    cx,
                ),
                button(
                    "equation-latex",
                    "LaTeX",
                    i18n::text("equation.latex_title"),
                    format == LinearFormat::LaTeX,
                    action(EquationAction::Format(LinearFormat::LaTeX)),
                    cx,
                ),
            ]),
            row(vec![
                button(
                    "equation-professional",
                    i18n::text("equation.professional"),
                    i18n::text("equation.professional_title"),
                    false,
                    action(EquationAction::Professional),
                    cx,
                ),
                button(
                    "equation-linear",
                    i18n::text("equation.linear"),
                    i18n::text("equation.linear_title"),
                    false,
                    action(EquationAction::Linear),
                    cx,
                ),
                button(
                    "equation-normal-text",
                    i18n::text("equation.normal_text"),
                    i18n::text("equation.normal_text_title"),
                    normal,
                    action(EquationAction::NormalText),
                    cx,
                ),
            ]),
        ],
    ));
    // 記号と特殊文字: the category's first symbols, all of them in the list.
    let category = view.equation.symbols.min(SYMBOL_CATEGORIES.len() - 1);
    let (_, symbols) = SYMBOL_CATEGORIES[category];
    let glyphs: Vec<AnyElement> = symbols
        .chars()
        .take(16)
        .enumerate()
        .map(|(index, ch)| symbol_button(("equation-symbol", index), ch, cx))
        .collect();
    let symbols_open = is_open(EquationPopup::Symbols);
    let symbols_menu = symbols_open.then(|| symbols_menu(category, cx));
    let mut first = glyphs;
    let second = first.split_off(8.min(first.len()));
    groups.push(group(
        i18n::text("equation.group.symbols"),
        vec![
            row(first),
            row({
                let mut second = second;
                second.push(dropdown(
                    menu_button(
                        "equation-symbols-more",
                        "",
                        i18n::text("equation.symbols.more"),
                        EquationPopup::Symbols,
                        symbols_open,
                        cx,
                    ),
                    symbols_menu,
                ));
                second
            }),
        ],
    ));
    // 構造.
    let mut structures = Vec::new();
    for structure in StructureGroup::ALL {
        let open = is_open(EquationPopup::Structure(structure));
        let menu = open.then(|| structure_menu(structure, cx));
        structures.push(dropdown(
            menu_button(
                SharedString::from(format!("equation-structure-{structure:?}")),
                i18n::text(structure.key()),
                i18n::text(structure.key()),
                EquationPopup::Structure(structure),
                open,
                cx,
            ),
            menu,
        ));
    }
    let rest = structures.split_off(6);
    groups.push(group(
        i18n::text("equation.group.structures"),
        vec![row(structures), row(rest)],
    ));
    let table = editor.table_context();
    groups.extend(layout_groups(table.as_ref(), cx));
    groups.push(group(
        i18n::text("equation.group.close"),
        vec![row(vec![button(
            "equation-close",
            i18n::text("equation.close"),
            i18n::text("equation.close"),
            false,
            action(EquationAction::Exit),
            cx,
        )])],
    ));
    groups
}

/// 配置 and, in a matrix or an equation array, 行列: line breaks, alignment
/// points, rows, columns and column justification.
fn layout_groups(
    table: Option<&jade_office_model::word::equation::MathTableContext>,
    cx: &mut Context<DocumentView>,
) -> Vec<AnyElement> {
    let mut groups = vec![group(
        i18n::text("equation.group.layout"),
        vec![row(vec![
            button(
                "equation-line-break",
                i18n::text("equation.line_break"),
                i18n::text("equation.line_break"),
                false,
                action(EquationAction::LineBreak),
                cx,
            ),
            button(
                "equation-align-point",
                i18n::text("equation.align_point"),
                i18n::text("equation.align_point"),
                false,
                action(EquationAction::AlignPoint),
                cx,
            ),
        ])],
    )];
    if let Some(table) = table {
        let mut rows = vec![row(vec![
            button(
                "equation-row-above",
                i18n::text("equation.row_above"),
                i18n::text("equation.row_above"),
                false,
                action(EquationAction::InsertRow(false)),
                cx,
            ),
            button(
                "equation-row-below",
                i18n::text("equation.row_below"),
                i18n::text("equation.row_below"),
                false,
                action(EquationAction::InsertRow(true)),
                cx,
            ),
            button(
                "equation-delete-row",
                i18n::text("equation.delete_row"),
                i18n::text("equation.delete_row"),
                false,
                action(EquationAction::DeleteRow),
                cx,
            ),
        ])];
        if table.matrix {
            rows.push(row(vec![
                button(
                    "equation-column-left",
                    i18n::text("equation.column_left"),
                    i18n::text("equation.column_left"),
                    false,
                    action(EquationAction::InsertColumn(false)),
                    cx,
                ),
                button(
                    "equation-column-right",
                    i18n::text("equation.column_right"),
                    i18n::text("equation.column_right"),
                    false,
                    action(EquationAction::InsertColumn(true)),
                    cx,
                ),
                button(
                    "equation-delete-column",
                    i18n::text("equation.delete_column"),
                    i18n::text("equation.delete_column"),
                    false,
                    action(EquationAction::DeleteColumn),
                    cx,
                ),
            ]));
            rows.push(row([
                ("left", "equation.justify_left"),
                ("center", "equation.justify_center"),
                ("right", "equation.justify_right"),
            ]
            .into_iter()
            .map(|(jc, key)| {
                button(
                    SharedString::from(format!("equation-justify-{jc}")),
                    i18n::text(key),
                    i18n::text(key),
                    table.justification == jc,
                    action(EquationAction::ColumnJustification(jc)),
                    cx,
                )
            })
            .collect()));
        }
        groups.push(group(i18n::text("equation.group.matrix"), rows));
    }
    groups
}

fn symbol_button(
    id: impl Into<gpui::ElementId>,
    ch: char,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded_sm()
        .text_sm()
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|b| b.bg(theme::hovered()))
        .on_click(listener(
            action(EquationAction::Insert(EquationInsert::Symbol(ch))),
            cx,
        ))
        .child(SharedString::from(ch.to_string()))
        .into_any_element()
}

fn symbols_menu(category: usize, cx: &mut Context<DocumentView>) -> AnyElement {
    let mut panel = menu_panel("equation-symbols").w(px(360.0));
    let mut tabs = Vec::new();
    for (index, (key, _)) in SYMBOL_CATEGORIES.iter().enumerate() {
        tabs.push(
            div()
                .id(("equation-symbol-category", index))
                .px_2()
                .py_0p5()
                .rounded_sm()
                .when(index == category, |t| t.bg(theme::selected()))
                .cursor_pointer()
                .hover(|t| t.bg(theme::hovered()))
                .on_click(listener(action(EquationAction::SymbolCategory(index)), cx))
                .child(i18n::text(key))
                .into_any_element(),
        );
    }
    panel = panel.child(div().flex().flex_wrap().gap_0p5().children(tabs));
    let (_, symbols) = SYMBOL_CATEGORIES[category];
    let grid: Vec<AnyElement> = symbols
        .chars()
        .enumerate()
        .map(|(index, ch)| symbol_button(("equation-symbol-all", index), ch, cx))
        .collect();
    panel
        .child(div().flex().flex_wrap().pt_1().children(grid))
        .into_any_element()
}

fn structure_menu(structure: StructureGroup, cx: &mut Context<DocumentView>) -> AnyElement {
    let mut panel = menu_panel("equation-structure").w(px(260.0));
    let mut items = Vec::new();
    for (index, item) in structure.items().iter().enumerate() {
        items.push(
            div()
                .id(("equation-structure-item", index))
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(56.0))
                .h(px(36.0))
                .px_1()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .cursor_pointer()
                .hover(|b| b.bg(theme::hovered()))
                .on_click(listener(
                    action(EquationAction::Insert(EquationInsert::Structure(
                        structure, index,
                    ))),
                    cx,
                ))
                .child(SharedString::from(*item))
                .into_any_element(),
        );
    }
    panel = panel.child(menu_caption(i18n::text(structure.key())));
    panel
        .child(div().flex().flex_wrap().gap_1().children(items))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{EquationAction, EquationPopup};
    use crate::ribbon::{Action, Popup};
    use crate::RibbonTab;
    use jade_office_model::word::equation::{EquationInsert, StructureGroup};
    use jade_office_model::word::WordSession;

    fn blank(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::Japanese,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "数式.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// 挿入 > 数式 opens a new equation with the 数式 tab; typing linear
    /// format builds it up; a structure from the tab goes in at the caret;
    /// arrows walk its arguments; the equation is saved as OMML.
    #[gpui::test]
    fn an_equation_is_inserted_typed_and_built_up(cx: &mut gpui::TestAppContext) {
        let (view, cx) = blank(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Equation(EquationAction::InsertNew), cx);
            assert!(view.equation.editor.is_some());
            assert!(view.ribbon_tabs().contains(&RibbonTab::Equation));
            assert!(view.ribbon_tab == RibbonTab::Equation);
        });
        cx.run_until_parked();
        cx.simulate_input("a/b ");
        view.update(cx, |view, _| {
            let editor = view.equation.editor.as_ref().unwrap();
            assert!(editor.omml().contains("<m:f>"), "{}", editor.omml());
        });
        view.update(cx, |view, cx| {
            view.run_ribbon(
                Action::Popup(Popup::Equation(EquationPopup::Structure(
                    StructureGroup::Radical,
                ))),
                cx,
            );
            view.run_ribbon(
                Action::Equation(EquationAction::Insert(EquationInsert::Structure(
                    StructureGroup::Radical,
                    0,
                ))),
                cx,
            );
        });
        cx.simulate_input("2");
        cx.simulate_keystrokes("right");
        cx.simulate_input("=c");
        view.update(cx, |view, _| {
            let editor = view.equation.editor.as_ref().unwrap();
            assert_eq!(editor.linear_text(), "a/b√2=c");
            let zones = WordSession::math_zones(&view.document);
            let zone = editor.zone(&zones).expect("drawn");
            assert!(editor.caret_geometry(zone).is_some());
        });
        // Escape leaves the equation; the document keeps it.
        cx.simulate_keystrokes("escape");
        view.update(cx, |view, _| {
            assert!(view.equation.editor.is_none());
            assert!(view.ribbon_tab != RibbonTab::Equation);
            let saved = view.document.export_docx().unwrap();
            let package = jade_opc::OpcPackage::open(&saved).unwrap();
            let xml = package.part_str("word/document.xml").unwrap().unwrap();
            assert!(
                xml.contains("<m:oMathPara") && xml.contains("<m:rad>"),
                "{xml}"
            );
        });
    }

    /// A built-in equation from 挿入's list goes in whole; Backspace twice
    /// takes a structure away; undo brings it back into the open editor.
    #[gpui::test]
    fn built_ins_backspace_and_undo(cx: &mut gpui::TestAppContext) {
        use crate::Undo;
        let (view, cx) = blank(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Equation(EquationAction::BuiltIn(0)), cx);
            let editor = view.equation.editor.as_ref().unwrap();
            assert!(editor.omml().contains("<m:rad>"));
            assert!(editor.omml().contains("<m:f>"));
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("backspace");
        cx.simulate_keystrokes("backspace");
        view.update(cx, |view, _| {
            let editor = view.equation.editor.as_ref().unwrap();
            assert!(!editor.omml().contains("<m:f>"), "{}", editor.omml());
        });
        cx.dispatch_action(Undo);
        view.update(cx, |view, _| {
            let editor = view.equation.editor.as_ref().expect("still editing");
            assert!(editor.omml().contains("<m:f>"));
        });
    }

    /// In a matrix the 数式 tab's 行列 tools add rows and columns and justify
    /// a column; ホーム's font size formats the equation.
    #[gpui::test]
    fn matrix_tools_and_home_formatting(cx: &mut gpui::TestAppContext) {
        let (view, cx) = blank(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Equation(EquationAction::InsertNew), cx);
            view.run_ribbon(
                Action::Equation(EquationAction::Insert(EquationInsert::Structure(
                    StructureGroup::Matrix,
                    4,
                ))),
                cx,
            );
            let table = view
                .equation
                .editor
                .as_ref()
                .unwrap()
                .table_context()
                .expect("in the matrix");
            assert_eq!((table.rows, table.columns), (2, 2));
            view.run_ribbon(Action::Equation(EquationAction::InsertRow(true)), cx);
            view.run_ribbon(Action::Equation(EquationAction::InsertColumn(true)), cx);
            view.run_ribbon(
                Action::Equation(EquationAction::ColumnJustification("right")),
                cx,
            );
            view.run_ribbon(Action::FontSize(16.0), cx);
            let editor = view.equation.editor.as_ref().unwrap();
            let table = editor.table_context().unwrap();
            assert_eq!((table.rows, table.columns), (3, 3));
            assert_eq!(table.justification, "right");
            let saved = view.document.export_docx().unwrap();
            let package = jade_opc::OpcPackage::open(&saved).unwrap();
            let xml = package.part_str("word/document.xml").unwrap().unwrap();
            assert_eq!(xml.matches("<m:mr>").count(), 3, "{xml}");
            assert!(xml.contains(r#"<m:mcJc m:val="right"/>"#));
        });
    }
}

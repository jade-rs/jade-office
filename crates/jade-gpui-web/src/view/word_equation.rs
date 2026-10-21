//! 数式 in the browser build: 挿入 > 数式 and its built-in list, the 数式
//! tab while an equation is edited, and the editing — the native shell's
//! `word_equation`, over the browser's Word document. Both run
//! `jade-office-model`'s [`EquationEditor`].

use gpui::{div, prelude::*, px, rgb, AnyElement, Context, KeyDownEvent, SharedString};
use jade_web::word::equation::{
    built_in_equation, insert_new_equation, zone_at, zone_for, EquationEditor, EquationFormat,
    EquationInsert, LinearFormat, StructureGroup, BUILT_IN_EQUATIONS, SYMBOL_CATEGORIES,
};
use jade_web::word::WordSession;

use super::{DocView, Document};
use crate::ribbon::{
    dropdown, group, listener, menu_caption, menu_panel, menu_row, row, Action, Popup, Tab,
};
use crate::tooltip::Tooltip;
use crate::{i18n, palette as theme};

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
    InsertNew,
    BuiltIn(usize),
    Insert(EquationInsert),
    Professional,
    Linear,
    Format(LinearFormat),
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

/// What the 数式 tab shows as current.
#[derive(Clone, Debug)]
pub struct EquationTabState {
    pub format: LinearFormat,
    pub normal_text: bool,
    pub symbols: usize,
    /// The matrix or equation array the caret is in.
    pub table: Option<jade_web::word::equation::MathTableContext>,
}

fn action(action: EquationAction) -> Action {
    Action::Equation(action)
}

/// An equation's frame, its empty arguments and its caret, on a page.
pub struct EquationOverlay {
    pub page: usize,
    pub frame: (f32, f32, f32, f32),
    pub placeholders: Vec<(f32, f32, f32, f32)>,
    pub caret: Option<(f32, f32, f32)>,
    pub empty: bool,
}

impl DocView {
    fn set_ribbon_tab(&mut self, tab: Tab) {
        if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.tab = tab;
            ribbon.popup = None;
        }
    }

    /// Runs a 数式 control.
    pub(crate) fn equation_action(&mut self, act: EquationAction, cx: &mut Context<Self>) {
        match act {
            EquationAction::InsertNew => self.insert_equation(None, cx),
            EquationAction::BuiltIn(index) => {
                if let Some(editor) = self.equation.as_mut() {
                    editor.insert(&EquationInsert::BuiltIn(index));
                    self.commit_equation(cx);
                } else {
                    let equation = built_in_equation(index);
                    self.insert_equation(equation.as_ref(), cx);
                }
            }
            EquationAction::Insert(item) => {
                if let Some(editor) = self.equation.as_mut() {
                    editor.insert(&item);
                    self.commit_equation(cx);
                }
            }
            EquationAction::Professional => {
                if let Some(editor) = self.equation.as_mut() {
                    editor.to_professional_form();
                    self.commit_equation(cx);
                }
            }
            EquationAction::Linear => {
                if let Some(editor) = self.equation.as_mut() {
                    editor.to_linear_form();
                    self.commit_equation(cx);
                }
            }
            EquationAction::Format(format) => {
                if let Some(editor) = self.equation.as_mut() {
                    editor.format = format;
                }
            }
            EquationAction::NormalText => {
                if let Some(editor) = self.equation.as_mut() {
                    editor.normal_text = !editor.normal_text;
                }
            }
            EquationAction::SymbolCategory(index) => self.equation_symbols = index,
            EquationAction::InsertRow(_)
            | EquationAction::InsertColumn(_)
            | EquationAction::DeleteRow
            | EquationAction::DeleteColumn
            | EquationAction::ColumnJustification(_)
            | EquationAction::LineBreak
            | EquationAction::AlignPoint => {
                let done = self.equation.as_mut().is_some_and(|editor| match &act {
                    EquationAction::InsertRow(below) => editor.insert_row(*below),
                    EquationAction::InsertColumn(after) => editor.insert_column(*after),
                    EquationAction::DeleteRow => editor.delete_row(),
                    EquationAction::DeleteColumn => editor.delete_column(),
                    EquationAction::ColumnJustification(jc) => editor.set_column_justification(jc),
                    EquationAction::LineBreak => editor.insert_line_break(),
                    _ => editor.set_alignment_point(),
                });
                if done {
                    self.commit_equation(cx);
                }
            }
            EquationAction::Exit => self.close_equation(),
        }
        cx.notify();
    }

    fn insert_equation(
        &mut self,
        content: Option<&jade_web::word::equation::MathElement>,
        cx: &mut Context<Self>,
    ) {
        self.close_equation();
        let Some(document) = self.word_mut() else {
            return;
        };
        let at = document
            .caret()
            .or_else(|| document.selection().map(|(_, focus)| focus))
            .unwrap_or((0, 0));
        let inserted = insert_new_equation(document, at, content);
        self.refresh_repainted();
        match inserted {
            Ok(target) => {
                let opened = self.word_ref().and_then(|document| {
                    let zones = WordSession::math_zones(document);
                    zone_for(&zones, &target).and_then(|zone| EquationEditor::open(document, zone))
                });
                if let Some(editor) = opened {
                    self.equation = Some(Box::new(editor));
                    self.set_ribbon_tab(Tab::Equation);
                }
                self.word_result(Ok(()), cx);
            }
            Err(error) => self.word_result(Err(error), cx),
        }
    }

    /// ホーム's size, colour and bold while an equation is edited: they
    /// format the equation. Returns whether the action was taken.
    pub(crate) fn equation_home_format(&mut self, action: &Action, cx: &mut Context<Self>) -> bool {
        use jade_web::word::CharacterFormat;
        let format = match action {
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
            _ => return false,
        };
        let Some(editor) = self.equation.as_mut() else {
            return false;
        };
        editor.format_runs(format);
        self.commit_equation(cx);
        true
    }

    /// Writes the edited equation back and repaints.
    fn commit_equation(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.equation.clone() else {
            return;
        };
        let Some(document) = self.word_mut() else {
            return;
        };
        let result = editor.commit(document);
        self.refresh_repainted();
        self.word_result(result, cx);
    }

    /// Leaves the equation being edited.
    pub(crate) fn close_equation(&mut self) {
        if self.equation.take().is_some() {
            if let Document::Pages { ribbon, .. } = &mut self.document {
                if ribbon.tab == Tab::Equation {
                    ribbon.tab = Tab::Home;
                }
            }
        }
    }

    /// After undo or redo: the equation read again from the document.
    pub(crate) fn equation_after_history(&mut self) {
        let Some(editor) = self.equation.as_ref() else {
            return;
        };
        let refreshed = self.word_ref().and_then(|document| {
            let zones = WordSession::math_zones(document);
            editor
                .zone(&zones)
                .and_then(|zone| EquationEditor::open(document, zone))
        });
        self.equation = refreshed.map(Box::new);
    }

    /// Typed text into the equation being edited.
    pub(crate) fn equation_type(&mut self, text: &str, cx: &mut Context<Self>) {
        let Some(editor) = self.equation.as_mut() else {
            return;
        };
        for ch in text.chars() {
            editor.type_text(&ch.to_string());
        }
        self.commit_equation(cx);
    }

    /// The keys the equation editor takes while it is open.
    pub(crate) fn equation_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if self.equation.is_none() {
            return false;
        }
        let modifiers = event.keystroke.modifiers;
        if modifiers.platform || modifiers.control {
            return false;
        }
        let key = event.keystroke.key.as_str();
        let zones = self
            .word_ref()
            .map(WordSession::math_zones)
            .unwrap_or_default();
        let Some(editor) = self.equation.as_mut() else {
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
                self.close_equation();
                cx.notify();
                return true;
            }
            _ => return false,
        };
        if edited {
            self.commit_equation(cx);
        } else {
            cx.notify();
        }
        true
    }

    /// A press on page `index` at (`x`, `y`) on the page: into an equation
    /// edits it; elsewhere leaves the one being edited.
    pub(crate) fn equation_click(
        &mut self,
        index: usize,
        x: f64,
        y: f64,
        cx: &mut Context<Self>,
    ) -> bool {
        let zones = self
            .word_ref()
            .map(WordSession::math_zones)
            .unwrap_or_default();
        let Some(zone) = zone_at(&zones, index + 1, x, y).cloned() else {
            self.close_equation();
            return false;
        };
        let same = self.equation.as_ref().is_some_and(|editor| {
            editor
                .zone(&zones)
                .is_some_and(|z| z.paragraph == zone.paragraph && z.offset == zone.offset)
        });
        if !same {
            let opened = self
                .word_ref()
                .and_then(|document| EquationEditor::open(document, &zone));
            self.equation = opened.map(Box::new);
        }
        if let Some(editor) = self.equation.as_mut() {
            editor.click(&zone, x, y);
            self.set_ribbon_tab(Tab::Equation);
            cx.notify();
            return true;
        }
        false
    }

    /// What the page draws over an equation being edited, zoomed.
    pub(crate) fn equation_overlay(&self, zoom: f32) -> Option<EquationOverlay> {
        let editor = self.equation.as_ref()?;
        let document = self.word_ref()?;
        let zones = WordSession::math_zones(document);
        let zone = editor.zone(&zones)?;
        let z = |v: f64| v as f32 * zoom;
        let (frame, placeholders) = editor.frames(zone);
        Some(EquationOverlay {
            page: zone.page.saturating_sub(1),
            frame: (z(frame.x), z(frame.y), z(frame.width), z(frame.height)),
            placeholders: placeholders
                .iter()
                .map(|p| (z(p.x), z(p.y), z(p.width), z(p.height)))
                .collect(),
            caret: editor
                .caret_geometry(zone)
                .map(|c| (z(c.x), z(c.top), z(c.height))),
            empty: editor.is_empty(),
        })
    }

    /// What the 数式 tab shows, while an equation is edited.
    pub(crate) fn equation_tab_state(&self) -> Option<EquationTabState> {
        self.equation.as_ref().map(|editor| EquationTabState {
            format: editor.format,
            normal_text: editor.normal_text,
            symbols: self.equation_symbols,
            table: editor.table_context(),
        })
    }
}

/// The overlay's elements over a page.
pub fn overlay_elements(overlay: &EquationOverlay, zoom: f32) -> Vec<AnyElement> {
    let mut children = Vec::new();
    let (x, y, w, h) = overlay.frame;
    children.push(
        div()
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w))
            .h(px(h))
            .border_1()
            .border_color(rgb(0x9aa4b2))
            .into_any_element(),
    );
    for &(x, y, w, h) in &overlay.placeholders {
        children.push(
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(h))
                .border_1()
                .border_dashed()
                .border_color(rgb(0x7f8a99))
                .into_any_element(),
        );
    }
    if overlay.empty {
        children.push(
            div()
                .absolute()
                .left(px(x + 2.0))
                .top(px(y))
                .h(px(h))
                .flex()
                .items_center()
                .text_size(px(12.0 * zoom))
                .text_color(rgb(0x7f8a99))
                .whitespace_nowrap()
                .child(i18n::text("equation.placeholder"))
                .into_any_element(),
        );
    }
    if let Some((x, top, height)) = overlay.caret {
        children.push(
            div()
                .absolute()
                .left(px(x))
                .top(px(top))
                .w(px(1.5))
                .h(px(height.max(8.0)))
                .bg(theme::ink())
                .into_any_element(),
        );
    }
    children
}

fn button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    tooltip: &'static str,
    active: bool,
    on_click: Action,
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
    cx: &mut Context<DocView>,
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
                .text_color(theme::dim()),
        )
        .into_any_element()
}

fn built_ins_menu(cx: &mut Context<DocView>) -> AnyElement {
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
                        .text_color(theme::dim())
                        .child(SharedString::from(*text)),
                ),
        );
    }
    panel.into_any_element()
}

/// 挿入's 数式 group.
pub fn insert_group(popup: Option<Popup>, cx: &mut Context<DocView>) -> AnyElement {
    let open = popup == Some(Popup::Equation(EquationPopup::BuiltIns));
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

/// 配置 and, in a matrix or an equation array, 行列: line breaks, alignment
/// points, rows, columns and column justification.
fn layout_groups(
    table: Option<&jade_web::word::equation::MathTableContext>,
    cx: &mut Context<DocView>,
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
    cx: &mut Context<DocView>,
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

fn symbols_menu(category: usize, cx: &mut Context<DocView>) -> AnyElement {
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
    let (_, symbols) = SYMBOL_CATEGORIES[category];
    let grid: Vec<AnyElement> = symbols
        .chars()
        .enumerate()
        .map(|(index, ch)| symbol_button(("equation-symbol-all", index), ch, cx))
        .collect();
    menu_panel("equation-symbols")
        .w(px(360.0))
        .child(div().flex().flex_wrap().gap_0p5().children(tabs))
        .child(div().flex().flex_wrap().pt_1().children(grid))
        .into_any_element()
}

fn structure_menu(structure: StructureGroup, cx: &mut Context<DocView>) -> AnyElement {
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
    menu_panel("equation-structure")
        .w(px(260.0))
        .child(menu_caption(i18n::text(structure.key())))
        .child(div().flex().flex_wrap().gap_1().children(items))
        .into_any_element()
}

/// The 数式 tab's groups.
pub fn tab_groups(
    popup: Option<Popup>,
    state: EquationTabState,
    cx: &mut Context<DocView>,
) -> Vec<AnyElement> {
    let is_open = |p: EquationPopup| popup == Some(Popup::Equation(p));
    let mut groups = Vec::new();
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
    groups.push(group(
        i18n::text("equation.group.convert"),
        vec![
            row(vec![
                button(
                    "equation-unicode",
                    "Unicode",
                    i18n::text("equation.unicode_title"),
                    state.format == LinearFormat::UnicodeMath,
                    action(EquationAction::Format(LinearFormat::UnicodeMath)),
                    cx,
                ),
                button(
                    "equation-latex",
                    "LaTeX",
                    i18n::text("equation.latex_title"),
                    state.format == LinearFormat::LaTeX,
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
                    state.normal_text,
                    action(EquationAction::NormalText),
                    cx,
                ),
            ]),
        ],
    ));
    let category = state.symbols.min(SYMBOL_CATEGORIES.len() - 1);
    let (_, symbols) = SYMBOL_CATEGORIES[category];
    let mut first: Vec<AnyElement> = symbols
        .chars()
        .take(16)
        .enumerate()
        .map(|(index, ch)| symbol_button(("equation-symbol", index), ch, cx))
        .collect();
    let mut second = first.split_off(8.min(first.len()));
    let symbols_open = is_open(EquationPopup::Symbols);
    let menu = symbols_open.then(|| symbols_menu(category, cx));
    second.push(dropdown(
        menu_button(
            "equation-symbols-more",
            "",
            i18n::text("equation.symbols.more"),
            EquationPopup::Symbols,
            symbols_open,
            cx,
        ),
        menu,
    ));
    groups.push(group(
        i18n::text("equation.group.symbols"),
        vec![row(first), row(second)],
    ));
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
    groups.extend(layout_groups(state.table.as_ref(), cx));
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

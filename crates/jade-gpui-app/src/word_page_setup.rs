//! Word's ページ設定 dialog — 文字数と行数, 余白, 用紙, その他 — with its
//! 行番号 panel, the 段組み and ページ番号の書式 dialogs, and the レイアウト
//! tab's menus that lead to them: 行番号, 文字列の方向, the breaks. The
//! dialogs' state and what applying them does are `jade-office-model`'s
//! (`word::page_setup`), which the browser shell draws too.

use gpui::{div, prelude::*, px, AnyElement, Context, KeyDownEvent, MouseButton, SharedString};
use jade_office_model::word::page::Paper;
use jade_office_model::word::page_setup::{
    points_to_mm, ApplyTo, BorderSetting, ColumnPreset, ColumnsDialog, CoverPage, Field, GridMode,
    LineNumbersPreset, MultiplePages, PageBorderDialog, PageBreak, PageNumberDialog,
    PageSetupDialog, SetupTab, WordPageSetup, CHAPTER_SEPARATORS, LINE_RESTARTS,
    PAGE_BORDER_STYLES, PAGE_BORDER_WIDTHS, PAGE_COLORS, PAGE_NUMBER_FORMATS, SECTION_STARTS,
    VERTICAL_ALIGNMENTS,
};

use crate::ribbon::{dropdown, group, listener, menu_panel, menu_row, row, Action, Popup};
use crate::tooltip::Tooltip;
use crate::{i18n, theme, DocumentView};

/// What the レイアウト tab's page controls do beyond `PageCommand`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SetupAction {
    /// The ページ設定 dialog, on a tab.
    Open(SetupTab),
    /// 段組みの詳細設定.
    OpenColumns,
    /// ページ番号の書式設定.
    OpenPageNumbers,
    Break(PageBreak),
    Columns(ColumnPreset),
    LineNumbers(LineNumbersPreset),
    /// 文字列の方向: 縦書き or 横書き.
    Vertical(bool),
    /// 段落の方向: 右から左 or 左から右.
    RightToLeft(bool),
    /// グリッドに合わせる for the selected paragraphs.
    SnapToGrid(bool),
    /// ページ罫線.
    OpenBorders,
    /// ページの色, `RRGGBB`, or 色なし.
    PageColor(Option<&'static str>),
    /// 表紙.
    Cover(CoverPage),
}

/// A number field of one of the dialogs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Setup(Field),
    ColumnCount,
    ColumnWidth(usize),
    ColumnSpace(usize),
    PageStart,
}

/// The dialog open over the pages.
pub enum PageDialog {
    Setup(PageSetupDialog),
    Columns(ColumnsDialog),
    PageNumbers(PageNumberDialog),
    Borders(PageBorderDialog),
}

/// The open dialog and the number being typed into one of its fields.
pub struct PageDialogState {
    pub dialog: PageDialog,
    /// The field, what has been typed, and whether typing has begun (the
    /// first character replaces the value, as in Word's fields).
    pub typing: Option<(Key, String, bool)>,
    /// Why OK did nothing, when it did nothing.
    pub problem: Option<&'static str>,
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// A choice in a row of choices: highlighted when chosen.
fn chip(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when(on, |chip| chip.bg(theme::selected()))
        .when(!enabled, |chip| chip.text_color(theme::text_faint()))
        .when(enabled, |chip| {
            chip.cursor_pointer()
                .hover(|chip| chip.bg(theme::hovered()))
        })
        .child(label.into())
}

/// A check box and its label.
fn check(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .cursor_pointer()
        .child(
            div()
                .size(px(12.0))
                .rounded_sm()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .when(on, |box_| box_.bg(theme::accent())),
        )
        .child(label)
}

/// A labelled line of the dialog.
fn line(label: &'static str) -> gpui::Div {
    div().flex().flex_wrap().items_center().gap_1().child(
        div()
            .w(px(104.0))
            .text_color(theme::text_dim())
            .child(t(label)),
    )
}

fn heading(label: &'static str) -> gpui::Div {
    div()
        .pt_1()
        .text_color(theme::text_faint())
        .border_b_1()
        .border_color(theme::border())
        .child(t(label))
}

fn number_text(value: f32, decimals: bool) -> String {
    if decimals {
        let text = format!("{value:.1}");
        text.trim_end_matches(".0").to_string()
    } else {
        format!("{}", value.round() as i64)
    }
}

impl PageDialogState {
    fn value(&self, key: Key) -> f32 {
        match (&self.dialog, key) {
            (PageDialog::Setup(dialog), Key::Setup(field)) => dialog.value(field),
            (PageDialog::Columns(dialog), Key::ColumnCount) => dialog.columns.count as f32,
            (PageDialog::Columns(dialog), Key::ColumnWidth(index)) => dialog
                .widths()
                .get(index)
                .map_or(0.0, |column| points_to_mm(column.width)),
            (PageDialog::Columns(dialog), Key::ColumnSpace(index)) => dialog
                .widths()
                .get(index)
                .map_or(0.0, |column| points_to_mm(column.space)),
            (PageDialog::PageNumbers(dialog), Key::PageStart) => {
                dialog.numbering.start.unwrap_or(1) as f32
            }
            _ => 0.0,
        }
    }

    fn decimals(key: Key) -> bool {
        match key {
            Key::Setup(field) => {
                field.is_length() || matches!(field, Field::CharPitch | Field::LinePitch)
            }
            Key::ColumnWidth(_) | Key::ColumnSpace(_) => true,
            Key::ColumnCount | Key::PageStart => false,
        }
    }

    fn text(&self, key: Key) -> String {
        match self.typing.as_ref() {
            Some((typing, text, true)) if *typing == key => text.clone(),
            _ => number_text(self.value(key), Self::decimals(key)),
        }
    }

    fn set(&mut self, key: Key, value: f32) {
        if !value.is_finite() {
            return;
        }
        match (&mut self.dialog, key) {
            (PageDialog::Setup(dialog), Key::Setup(field)) => dialog.set_value(field, value),
            (PageDialog::Columns(dialog), Key::ColumnCount) => {
                dialog.set_count(value.round().max(1.0) as u32)
            }
            (PageDialog::Columns(dialog), Key::ColumnWidth(index)) => {
                dialog.set_width(index, value)
            }
            (PageDialog::Columns(dialog), Key::ColumnSpace(index)) => {
                dialog.set_space(index, value)
            }
            (PageDialog::PageNumbers(dialog), Key::PageStart) => {
                dialog.set_start(value.round().max(1.0) as u32)
            }
            _ => {}
        }
    }

    /// What was typed becomes the field's value.
    pub fn commit(&mut self) {
        if let Some((key, text, started)) = self.typing.take() {
            if started {
                if let Ok(value) = text.parse::<f32>() {
                    self.set(key, value);
                }
            }
        }
    }

    fn step(&mut self, key: Key, up: bool) {
        self.commit();
        let step = match key {
            Key::Setup(Field::CharPitch | Field::LinePitch) => 0.5,
            _ => 1.0,
        };
        let value = self.value(key) + if up { step } else { -step };
        self.set(key, value);
    }
}

/// The 区切り menu's breaks inside a section, above its section breaks.
pub fn page_break_rows(cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    [
        (PageBreak::Page, "setup.break.page"),
        (PageBreak::Column, "setup.break.column"),
        (PageBreak::TextWrapping, "setup.break.text_wrapping"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (kind, label))| {
        menu_row(
            ("page-break", index),
            t(label),
            false,
            Action::Setup(SetupAction::Break(kind)),
            cx,
        )
    })
    .collect()
}

/// The 段組み menu's presets and its 段組みの詳細設定.
pub fn column_rows(current: u32, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let mut rows: Vec<AnyElement> = [
        (ColumnPreset::One, "setup.columns.one", current == 1),
        (ColumnPreset::Two, "setup.columns.two", current == 2),
        (ColumnPreset::Three, "setup.columns.three", current == 3),
        (ColumnPreset::Left, "setup.columns.left", false),
        (ColumnPreset::Right, "setup.columns.right", false),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (preset, label, on))| {
        menu_row(
            ("column-preset", index),
            t(label),
            on,
            Action::Setup(SetupAction::Columns(preset)),
            cx,
        )
    })
    .collect();
    rows.push(menu_row(
        "columns-more",
        t("setup.columns.more"),
        false,
        Action::Setup(SetupAction::OpenColumns),
        cx,
    ));
    rows
}

/// A text button with a chevron that opens a list.
fn list_button(
    id: &'static str,
    label: &'static str,
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
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .whitespace_nowrap()
        .cursor_pointer()
        .when(open, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .on_click(listener(Action::Popup(popup), cx))
        .child(label)
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element()
}

fn text_button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    active: bool,
    action: Action,
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
        .on_click(listener(action, cx))
        .child(label)
        .into_any_element()
}

impl DocumentView {
    /// レイアウト's 行番号, 文字列の方向 and the ページ設定 dialog launcher.
    pub(crate) fn page_setup_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let open = self.ribbon.popup;
        let line_numbers = self.document.line_numbers_preset();
        let vertical = self.document.vertical_text();
        let rtl = self.document.paragraph_rtl();
        let snap = self.document.snap_to_grid();
        let line_menu = (open == Some(Popup::LineNumbers)).then(|| {
            menu_panel("line-numbers-menu")
                .w(px(220.0))
                .children(
                    [
                        (LineNumbersPreset::None, "setup.lines.none"),
                        (LineNumbersPreset::Continuous, "setup.lines.continuous"),
                        (LineNumbersPreset::RestartEachPage, "setup.lines.new_page"),
                        (
                            LineNumbersPreset::RestartEachSection,
                            "setup.lines.new_section",
                        ),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, (preset, label))| {
                        menu_row(
                            ("line-numbers", index),
                            t(label),
                            line_numbers == preset,
                            Action::Setup(SetupAction::LineNumbers(preset)),
                            cx,
                        )
                    }),
                )
                .child(menu_row(
                    "line-numbers-options",
                    t("setup.lines.options"),
                    false,
                    Action::Setup(SetupAction::Open(SetupTab::Other)),
                    cx,
                ))
                .into_any_element()
        });
        let direction_menu = (open == Some(Popup::TextDirection)).then(|| {
            menu_panel("text-direction-menu")
                .w(px(200.0))
                .child(menu_row(
                    "direction-horizontal",
                    t("setup.direction.horizontal"),
                    !vertical,
                    Action::Setup(SetupAction::Vertical(false)),
                    cx,
                ))
                .child(menu_row(
                    "direction-vertical",
                    t("setup.direction.vertical"),
                    vertical,
                    Action::Setup(SetupAction::Vertical(true)),
                    cx,
                ))
                .child(menu_row(
                    "direction-options",
                    t("setup.direction.options"),
                    false,
                    Action::Setup(SetupAction::Open(SetupTab::Grid)),
                    cx,
                ))
                .into_any_element()
        });
        vec![group(
            t("setup.group.text"),
            vec![
                row(vec![
                    dropdown(
                        list_button(
                            "text-direction",
                            t("setup.direction"),
                            open == Some(Popup::TextDirection),
                            Popup::TextDirection,
                            cx,
                        ),
                        direction_menu,
                    ),
                    dropdown(
                        list_button(
                            "line-numbers",
                            t("setup.lines"),
                            open == Some(Popup::LineNumbers),
                            Popup::LineNumbers,
                            cx,
                        ),
                        line_menu,
                    ),
                ]),
                row(vec![
                    text_button(
                        "paragraph-ltr",
                        t("setup.ltr"),
                        t("setup.ltr_title"),
                        !rtl,
                        Action::Setup(SetupAction::RightToLeft(false)),
                        cx,
                    ),
                    text_button(
                        "paragraph-rtl",
                        t("setup.rtl"),
                        t("setup.rtl_title"),
                        rtl,
                        Action::Setup(SetupAction::RightToLeft(true)),
                        cx,
                    ),
                    text_button(
                        "snap-to-grid",
                        t("setup.snap"),
                        t("setup.snap_title"),
                        snap,
                        Action::Setup(SetupAction::SnapToGrid(!snap)),
                        cx,
                    ),
                    text_button(
                        "page-setup-dialog",
                        t("setup.open"),
                        t("setup.title"),
                        false,
                        Action::Setup(SetupAction::Open(SetupTab::Grid)),
                        cx,
                    ),
                ]),
            ],
        )]
    }

    /// Runs a レイアウト page control.
    pub(crate) fn run_setup_action(
        &mut self,
        action: SetupAction,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let document = &mut self.document;
        match action {
            SetupAction::Open(tab) => {
                self.page_dialog = Some(PageDialogState {
                    dialog: PageDialog::Setup(document.page_setup_dialog(tab)),
                    typing: None,
                    problem: None,
                });
                cx.notify();
                Ok(())
            }
            SetupAction::OpenColumns => {
                let setup = document.page_setup_dialog(SetupTab::Grid);
                self.page_dialog = Some(PageDialogState {
                    dialog: PageDialog::Columns(ColumnsDialog::open(&setup)),
                    typing: None,
                    problem: None,
                });
                cx.notify();
                Ok(())
            }
            SetupAction::OpenPageNumbers => {
                let numbering = document.section_layout().page_numbering;
                self.page_dialog = Some(PageDialogState {
                    dialog: PageDialog::PageNumbers(PageNumberDialog::open(numbering)),
                    typing: None,
                    problem: None,
                });
                cx.notify();
                Ok(())
            }
            SetupAction::Break(kind) => {
                // Where the caret lands is past the break: the start of the
                // text after it.
                let done = document.insert_page_break(kind);
                if done.is_ok() {
                    if let Some(range) = document.selection_range() {
                        self.insert_at = (range.focus.paragraph, range.focus.offset);
                    }
                }
                done
            }
            SetupAction::Columns(preset) => document.set_column_preset(preset),
            SetupAction::LineNumbers(preset) => document.set_line_numbers(preset),
            SetupAction::Vertical(vertical) => document.set_vertical_text(vertical),
            SetupAction::RightToLeft(rtl) => document.set_paragraph_rtl(rtl),
            SetupAction::SnapToGrid(snap) => document.set_snap_to_grid(snap),
            SetupAction::OpenBorders => {
                let setup = document.page_setup_dialog(SetupTab::Other);
                self.page_dialog = Some(PageDialogState {
                    dialog: PageDialog::Borders(PageBorderDialog::open(&setup)),
                    typing: None,
                    problem: None,
                });
                cx.notify();
                Ok(())
            }
            SetupAction::PageColor(color) => document.set_page_color(color),
            SetupAction::Cover(cover) => {
                let japanese = i18n::language() == i18n::Language::Japanese;
                let done = document.insert_cover_page(cover, japanese);
                if done.is_ok() {
                    self.insert_at = (0, 0);
                }
                done
            }
        }
    }

    /// OK: what the open dialog changed lands on its sections.
    fn apply_page_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(mut state) = self.page_dialog.take() else {
            return;
        };
        state.commit();
        let result = match &state.dialog {
            PageDialog::Setup(dialog) => {
                if let Some(problem) = dialog.problem() {
                    state.problem = Some(problem);
                    self.page_dialog = Some(state);
                    return cx.notify();
                }
                self.document.apply_page_setup(dialog)
            }
            PageDialog::Columns(dialog) => self.document.apply_columns(dialog),
            PageDialog::PageNumbers(dialog) => self.document.apply_page_numbering(dialog),
            PageDialog::Borders(dialog) => self.document.apply_page_borders(dialog),
        };
        match result {
            Ok(()) => {
                self.dirty = true;
                self.table_status = None;
                if self.document.selection().is_none() {
                    self.caret = self
                        .document
                        .place_caret(self.insert_at.0, self.insert_at.1);
                }
                self.rebuild_all_images();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    fn edit_page_dialog(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut PageDialogState),
    ) {
        if let Some(state) = self.page_dialog.as_mut() {
            edit(state);
            state.problem = None;
            cx.notify();
        }
    }

    fn edit_setup(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut PageSetupDialog)) {
        self.edit_page_dialog(cx, |state| {
            state.commit();
            if let PageDialog::Setup(dialog) = &mut state.dialog {
                edit(dialog);
            }
        });
    }

    /// Keys while a dialog is open: digits into the field being typed,
    /// Enter applies (or ends the typing), Escape cancels.
    pub(crate) fn page_dialog_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let Some(state) = self.page_dialog.as_mut() else {
            return false;
        };
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => {
                if state.typing.take().is_none() {
                    self.page_dialog = None;
                }
            }
            "enter" | "return" => {
                if state.typing.is_some() {
                    state.commit();
                } else {
                    self.apply_page_dialog(cx);
                    return true;
                }
            }
            "tab" => state.commit(),
            "backspace" => {
                if let Some((_, text, started)) = state.typing.as_mut() {
                    *started = true;
                    text.pop();
                }
            }
            "up" | "down" => {
                if let Some((key, _, _)) = state.typing {
                    state.step(key, event.keystroke.key == "up");
                    state.typing = Some((key, String::new(), false));
                }
            }
            _ => {
                let typed = event
                    .keystroke
                    .key_char
                    .clone()
                    .unwrap_or_else(|| event.keystroke.key.clone());
                let Some((_, text, started)) = state.typing.as_mut() else {
                    // The dialog is modal: nothing reaches the document.
                    return true;
                };
                for ch in typed.chars() {
                    let ch = match ch {
                        '０'..='９' => char::from(b'0' + (ch as u32 - '０' as u32) as u8),
                        '．' => '.',
                        other => other,
                    };
                    if ch.is_ascii_digit() || ch == '.' {
                        if !*started {
                            text.clear();
                            *started = true;
                        }
                        text.push(ch);
                    }
                }
            }
        }
        cx.notify();
        true
    }

    /// A number field: its value (or what is typed, with a caret), and the
    /// spinner beside it.
    fn number_field(
        &self,
        key: Key,
        unit: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(state) = self.page_dialog.as_ref() else {
            return div().into_any_element();
        };
        let focused = state
            .typing
            .as_ref()
            .is_some_and(|(typing, _, _)| *typing == key);
        let id = format!("{key:?}");
        div()
            .flex()
            .items_center()
            .gap_0p5()
            .child(
                div()
                    .id(SharedString::from(format!("field-{id}")))
                    .w(px(64.0))
                    .h(px(20.0))
                    .px_1()
                    .flex()
                    .items_center()
                    .justify_end()
                    .rounded_sm()
                    .border_1()
                    .border_color(if focused {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(theme::canvas())
                    .when(!enabled, |field| field.text_color(theme::text_faint()))
                    .when(enabled, |field| {
                        field
                            .cursor_text()
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.edit_page_dialog(cx, |state| {
                                    state.commit();
                                    state.typing = Some((key, String::new(), false));
                                })
                            }))
                    })
                    .child(state.text(key))
                    .when(focused, |field| {
                        field.child(div().w(px(1.0)).h(px(12.0)).bg(theme::accent()))
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id(SharedString::from(format!("up-{id}")))
                            .px_0p5()
                            .text_size(px(8.0))
                            .when(enabled, |spin| {
                                spin.cursor_pointer()
                                    .hover(|spin| spin.bg(theme::hovered()))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.edit_page_dialog(cx, |state| state.step(key, true))
                                    }))
                            })
                            .child("▲"),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("down-{id}")))
                            .px_0p5()
                            .text_size(px(8.0))
                            .when(enabled, |spin| {
                                spin.cursor_pointer()
                                    .hover(|spin| spin.bg(theme::hovered()))
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.edit_page_dialog(cx, |state| state.step(key, false))
                                    }))
                            })
                            .child("▼"),
                    ),
            )
            .child(div().text_color(theme::text_dim()).child(t(unit)))
            .into_any_element()
    }

    fn setup_field(&self, field: Field, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        self.number_field(Key::Setup(field), field.unit_key(), enabled, cx)
    }

    /// The dialog over the pages, while one is open.
    pub(crate) fn page_dialog_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.page_dialog.as_ref()?;
        let (title, body, apply_to): (&'static str, AnyElement, Option<(ApplyTo, Vec<ApplyTo>)>) =
            match &state.dialog {
                PageDialog::Setup(dialog) => (
                    "setup.title",
                    self.setup_body(dialog, cx),
                    Some((dialog.apply_to, dialog.apply_choices.clone())),
                ),
                PageDialog::Columns(dialog) => (
                    "setup.columns.title",
                    self.columns_body(dialog, cx),
                    Some((dialog.apply_to, dialog.apply_choices.clone())),
                ),
                PageDialog::PageNumbers(dialog) => (
                    "setup.numbers.title",
                    self.page_numbers_body(dialog, cx),
                    None,
                ),
                PageDialog::Borders(dialog) => (
                    "setup.borders.title",
                    self.borders_body(dialog, cx),
                    Some((dialog.apply_to, dialog.apply_choices.clone())),
                ),
            };
        let apply_row = apply_to.map(|(current, choices)| {
            line("setup.apply").children(choices.into_iter().enumerate().map(|(index, choice)| {
                chip(
                    ("apply-to", index),
                    t(choice.label_key()),
                    choice == current,
                    true,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.edit_page_dialog(cx, |state| match &mut state.dialog {
                        PageDialog::Setup(dialog) => dialog.apply_to = choice,
                        PageDialog::Columns(dialog) => dialog.apply_to = choice,
                        PageDialog::Borders(dialog) => dialog.apply_to = choice,
                        PageDialog::PageNumbers(_) => {}
                    })
                }))
            }))
        });
        Some(
            div()
                .id("page-dialog-backdrop")
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
                        .id("page-dialog")
                        .w(px(560.0))
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .rounded_md()
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::panel())
                        .shadow_lg()
                        .text_xs()
                        .text_color(theme::text())
                        .child(div().text_sm().child(t(title)))
                        .child(body)
                        .children(apply_row)
                        .children(
                            state.problem.map(|problem| {
                                div().text_color(gpui::rgb(0xD13438)).child(t(problem))
                            }),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(
                                    chip("page-dialog-cancel", t("format.cancel"), false, true)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.page_dialog = None;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    chip("page-dialog-ok", t("format.ok"), true, true).on_click(
                                        cx.listener(|view, _, _, cx| view.apply_page_dialog(cx)),
                                    ),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    fn setup_body(&self, dialog: &PageSetupDialog, cx: &mut Context<Self>) -> AnyElement {
        let tabs = SetupTab::ALL.into_iter().enumerate().map(|(index, tab)| {
            chip(
                ("setup-tab", index),
                t(tab.label_key()),
                dialog.tab == tab,
                true,
            )
            .on_click(
                cx.listener(move |view, _, _, cx| view.edit_setup(cx, |dialog| dialog.tab = tab)),
            )
        });
        let tabs: Vec<_> = tabs.collect();
        let body = match dialog.tab {
            SetupTab::Grid => self.grid_tab(dialog, cx),
            SetupTab::Margins => self.margins_tab(dialog, cx),
            SetupTab::Paper => self.paper_tab(dialog, cx),
            SetupTab::Other => self.other_tab(dialog, cx),
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().flex().gap_1().children(tabs))
            .child(
                div()
                    .min_h(px(250.0))
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .child(body),
            )
            .into_any_element()
    }

    fn grid_tab(&self, dialog: &PageSetupDialog, cx: &mut Context<Self>) -> gpui::Div {
        let vertical = dialog.layout.text_direction == "tbRl";
        let mode = dialog.grid_mode();
        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(heading("setup.direction.heading"))
            .child(
                line("setup.direction.label")
                    .child(
                        chip(
                            "direction-h",
                            t("setup.direction.horizontal"),
                            !vertical,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_setup(cx, |dialog| dialog.set_vertical(false))
                        })),
                    )
                    .child(
                        chip("direction-v", t("setup.direction.vertical"), vertical, true)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.edit_setup(cx, |dialog| dialog.set_vertical(true))
                            })),
                    ),
            )
            .child(line("setup.columns.count").child(self.setup_field(Field::Columns, true, cx)))
            .child(heading("setup.grid.heading"))
            .child(div().flex().flex_wrap().gap_1().children(
                GridMode::ALL.into_iter().enumerate().map(|(index, entry)| {
                    chip(("grid", index), t(entry.label_key()), mode == entry, true).on_click(
                        cx.listener(move |view, _, _, cx| {
                            view.edit_setup(cx, |dialog| dialog.set_grid_mode(entry))
                        }),
                    )
                }),
            ))
            .child(heading("setup.chars.heading"))
            .child(
                line("setup.chars")
                    .child(self.setup_field(Field::CharsPerLine, mode.sets_chars(), cx))
                    .child(div().w(px(16.0)))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("setup.char_pitch")),
                    )
                    .child(self.setup_field(Field::CharPitch, mode.sets_chars(), cx)),
            )
            .child(heading("setup.lines.heading"))
            .child(
                line("setup.lines_per_page")
                    .child(self.setup_field(Field::LinesPerPage, mode.sets_lines(), cx))
                    .child(div().w(px(16.0)))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("setup.line_pitch")),
                    )
                    .child(self.setup_field(Field::LinePitch, mode.sets_lines(), cx)),
            )
    }

    fn margins_tab(&self, dialog: &PageSetupDialog, cx: &mut Context<Self>) -> gpui::Div {
        let (top, bottom) = if dialog.outside_inside() {
            ("setup.margin.outside", "setup.margin.inside")
        } else {
            ("setup.margin.top", "setup.margin.bottom")
        };
        let (left, right) = if dialog.inside_outside() {
            ("setup.margin.inside", "setup.margin.outside")
        } else {
            ("setup.margin.left", "setup.margin.right")
        };
        let pages = dialog.multiple_pages();
        let gutter_side_fixed = pages != MultiplePages::Normal;
        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(heading("setup.margins.heading"))
            .child(
                line(top)
                    .child(self.setup_field(Field::MarginTop, true, cx))
                    .child(div().w(px(16.0)))
                    .child(
                        div()
                            .w(px(40.0))
                            .text_color(theme::text_dim())
                            .child(t(bottom)),
                    )
                    .child(self.setup_field(Field::MarginBottom, true, cx)),
            )
            .child(
                line(left)
                    .child(self.setup_field(Field::MarginLeft, true, cx))
                    .child(div().w(px(16.0)))
                    .child(
                        div()
                            .w(px(40.0))
                            .text_color(theme::text_dim())
                            .child(t(right)),
                    )
                    .child(self.setup_field(Field::MarginRight, true, cx)),
            )
            .child(
                line("setup.gutter")
                    .child(self.setup_field(Field::Gutter, true, cx))
                    .child(div().w(px(16.0)))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("setup.gutter_side")),
                    )
                    .child(
                        chip(
                            "gutter-left",
                            t("setup.margin.left"),
                            !dialog.layout.gutter_at_top,
                            !gutter_side_fixed,
                        )
                        .when(!gutter_side_fixed, |chip| {
                            chip.on_click(cx.listener(|view, _, _, cx| {
                                view.edit_setup(cx, |dialog| dialog.layout.gutter_at_top = false)
                            }))
                        }),
                    )
                    .child(
                        chip(
                            "gutter-top",
                            t("setup.margin.top"),
                            dialog.layout.gutter_at_top,
                            !gutter_side_fixed,
                        )
                        .when(!gutter_side_fixed, |chip| {
                            chip.on_click(cx.listener(|view, _, _, cx| {
                                view.edit_setup(cx, |dialog| dialog.layout.gutter_at_top = true)
                            }))
                        }),
                    ),
            )
            .child(heading("setup.orientation.heading"))
            .child(
                line("setup.orientation")
                    .child(
                        chip(
                            "orient-portrait",
                            t("page.portrait"),
                            !dialog.layout.landscape,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_setup(cx, |dialog| dialog.set_landscape(false))
                        })),
                    )
                    .child(
                        chip(
                            "orient-landscape",
                            t("page.landscape"),
                            dialog.layout.landscape,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_setup(cx, |dialog| dialog.set_landscape(true))
                        })),
                    ),
            )
            .child(heading("setup.pages.heading"))
            .child(
                line("setup.pages").children(MultiplePages::ALL.into_iter().enumerate().map(
                    |(index, entry)| {
                        chip(("pages", index), t(entry.label_key()), pages == entry, true).on_click(
                            cx.listener(move |view, _, _, cx| {
                                view.edit_setup(cx, |dialog| dialog.set_multiple_pages(entry))
                            }),
                        )
                    },
                )),
            )
            .child(Self::preview(dialog))
    }

    /// The プレビュー: the page, its margins, and the gutter.
    fn preview(dialog: &PageSetupDialog) -> AnyElement {
        let layout = &dialog.layout;
        let scale = 70.0 / layout.width.max(layout.height);
        let width = layout.width * scale;
        let height = layout.height * scale;
        let gutter = layout.gutter * scale;
        let (gutter_left, gutter_top) = if layout.gutter_at_top {
            (0.0, gutter)
        } else {
            (gutter, 0.0)
        };
        div()
            .flex()
            .justify_center()
            .child(
                div()
                    .relative()
                    .w(px(width))
                    .h(px(height))
                    .bg(gpui::rgb(0xFFFFFF))
                    .border_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .absolute()
                            .left(px(layout.margin_left * scale + gutter_left))
                            .top(px(layout.margin_top * scale + gutter_top))
                            .w(px((width
                                - (layout.margin_left + layout.margin_right) * scale
                                - gutter_left)
                                .max(1.0)))
                            .h(px((height
                                - (layout.margin_top + layout.margin_bottom) * scale
                                - gutter_top)
                                .max(1.0)))
                            .border_1()
                            .border_color(gpui::rgb(0x9AA0A6)),
                    )
                    .when(gutter > 0.0, |page| {
                        page.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .when(layout.gutter_at_top, |bar| bar.w_full().h(px(gutter)))
                                .when(!layout.gutter_at_top, |bar| bar.h_full().w(px(gutter)))
                                .bg(gpui::rgba(0x0F6CBD44)),
                        )
                    }),
            )
            .into_any_element()
    }

    fn paper_tab(&self, dialog: &PageSetupDialog, cx: &mut Context<Self>) -> gpui::Div {
        let current = dialog.paper();
        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(heading("setup.paper.heading"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(Paper::ALL.into_iter().enumerate().map(|(index, paper)| {
                        chip(("paper", index), paper.name(), current == Some(paper), true).on_click(
                            cx.listener(move |view, _, _, cx| {
                                view.edit_setup(cx, |dialog| dialog.set_paper(paper))
                            }),
                        )
                    }))
                    .child(chip(
                        "paper-custom",
                        t("setup.paper.custom"),
                        current.is_none(),
                        false,
                    )),
            )
            .child(line("setup.paper.width").child(self.setup_field(Field::PaperWidth, true, cx)))
            .child(line("setup.paper.height").child(self.setup_field(Field::PaperHeight, true, cx)))
    }

    fn other_tab(&self, dialog: &PageSetupDialog, cx: &mut Context<Self>) -> gpui::Div {
        let layout = &dialog.layout;
        let mut tab = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(heading("setup.section.heading"))
            .child(
                line("setup.section.start").children(SECTION_STARTS.into_iter().enumerate().map(
                    |(index, (value, label))| {
                        chip(("start", index), t(label), layout.start == value, true).on_click(
                            cx.listener(move |view, _, _, cx| {
                                view.edit_setup(cx, |dialog| {
                                    dialog.layout.start = value.to_string()
                                })
                            }),
                        )
                    },
                )),
            )
            .child(heading("setup.header_footer.heading"))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        check(
                            "odd-even",
                            t("setup.header_footer.odd_even"),
                            layout.different_odd_even,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_setup(cx, |dialog| {
                                dialog.layout.different_odd_even = !dialog.layout.different_odd_even
                            })
                        })),
                    )
                    .child(
                        check(
                            "first-page",
                            t("setup.header_footer.first_page"),
                            layout.different_first_page,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_setup(cx, |dialog| {
                                dialog.layout.different_first_page =
                                    !dialog.layout.different_first_page
                            })
                        })),
                    ),
            )
            .child(
                line("setup.header_distance")
                    .child(self.setup_field(Field::HeaderDistance, true, cx))
                    .child(div().w(px(16.0)))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("setup.footer_distance")),
                    )
                    .child(self.setup_field(Field::FooterDistance, true, cx)),
            )
            .child(heading("setup.page.heading"))
            .child(
                line("setup.valign").children(VERTICAL_ALIGNMENTS.into_iter().enumerate().map(
                    |(index, (value, label))| {
                        chip(
                            ("valign", index),
                            t(label),
                            layout.vertical_alignment == value,
                            true,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.edit_setup(cx, |dialog| {
                                dialog.layout.vertical_alignment = value.to_string()
                            })
                        }))
                    },
                )),
            )
            .child(
                div().flex().gap_2().child(
                    chip(
                        "line-numbers-button",
                        t("setup.lines.button"),
                        dialog.line_numbers_open,
                        true,
                    )
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.edit_setup(cx, |dialog| {
                            dialog.line_numbers_open = !dialog.line_numbers_open
                        })
                    })),
                ),
            );
        if dialog.line_numbers_open {
            let numbering = &layout.line_numbering;
            let enabled = numbering.enabled;
            tab = tab
                .child(heading("setup.lines.title"))
                .child(
                    check("line-numbers-on", t("setup.lines.add"), enabled).on_click(cx.listener(
                        |view, _, _, cx| {
                            view.edit_setup(cx, |dialog| {
                                dialog.layout.line_numbering.enabled =
                                    !dialog.layout.line_numbering.enabled
                            })
                        },
                    )),
                )
                .child(
                    line("setup.lines.start")
                        .child(self.setup_field(Field::LineStart, enabled, cx))
                        .child(div().w(px(16.0)))
                        .child(
                            div()
                                .text_color(theme::text_dim())
                                .child(t("setup.lines.count_by")),
                        )
                        .child(self.setup_field(Field::LineCountBy, enabled, cx)),
                )
                .child(
                    line("setup.lines.distance")
                        .child(self.setup_field(Field::LineDistance, enabled, cx))
                        .child(
                            div()
                                .text_color(theme::text_faint())
                                .child(t("setup.lines.auto")),
                        ),
                )
                .child(
                    line("setup.lines.restart").children(
                        LINE_RESTARTS
                            .into_iter()
                            .enumerate()
                            .map(|(index, (value, label))| {
                                chip(
                                    ("restart", index),
                                    t(label),
                                    numbering.restart == value,
                                    enabled,
                                )
                                .when(enabled, |chip| {
                                    chip.on_click(cx.listener(move |view, _, _, cx| {
                                        view.edit_setup(cx, |dialog| {
                                            dialog.layout.line_numbering.restart = value.to_string()
                                        })
                                    }))
                                })
                            }),
                    ),
                );
        }
        tab
    }

    /// デザイン's ページの背景: ページの色 and ページ罫線.
    pub(crate) fn design_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let open = self.ribbon.popup;
        let current = self.document.page_color();
        let color_menu = (open == Some(Popup::PageColor)).then(|| {
            menu_panel("page-color-menu")
                .w(px(196.0))
                .child(
                    div().flex().flex_wrap().gap_1().p_1().children(
                        PAGE_COLORS
                            .into_iter()
                            .enumerate()
                            .map(|(index, (color, label))| {
                                div()
                                    .id(("page-color", index))
                                    .size(px(20.0))
                                    .rounded_sm()
                                    .bg(gpui::rgb(
                                        u32::from_str_radix(color, 16).unwrap_or(0xFFFFFF),
                                    ))
                                    .border_1()
                                    .border_color(if current.as_deref() == Some(color) {
                                        theme::accent()
                                    } else {
                                        theme::border()
                                    })
                                    .cursor_pointer()
                                    .tooltip(Tooltip::text(t(label)))
                                    .on_click(listener(
                                        Action::Setup(SetupAction::PageColor(Some(color))),
                                        cx,
                                    ))
                            }),
                    ),
                )
                .child(menu_row(
                    "page-color-none",
                    t("setup.color.none"),
                    current.is_none(),
                    Action::Setup(SetupAction::PageColor(None)),
                    cx,
                ))
                .into_any_element()
        });
        vec![group(
            t("setup.group.background"),
            vec![row(vec![
                dropdown(
                    list_button(
                        "page-color",
                        t("setup.page_color"),
                        open == Some(Popup::PageColor),
                        Popup::PageColor,
                        cx,
                    ),
                    color_menu,
                ),
                text_button(
                    "page-borders",
                    t("setup.borders.open"),
                    t("setup.borders.title"),
                    false,
                    Action::Setup(SetupAction::OpenBorders),
                    cx,
                ),
            ])],
        )]
    }

    /// 挿入's 表紙 gallery.
    pub(crate) fn cover_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.ribbon.popup;
        let menu = (open == Some(Popup::Cover)).then(|| {
            menu_panel("cover-menu")
                .w(px(180.0))
                .children(
                    CoverPage::ALL
                        .into_iter()
                        .enumerate()
                        .map(|(index, cover)| {
                            menu_row(
                                ("cover", index),
                                t(cover.label_key()),
                                false,
                                Action::Setup(SetupAction::Cover(cover)),
                                cx,
                            )
                        }),
                )
                .into_any_element()
        });
        group(
            t("setup.cover.group"),
            vec![row(vec![dropdown(
                list_button(
                    "cover-page",
                    t("setup.cover"),
                    open == Some(Popup::Cover),
                    Popup::Cover,
                    cx,
                ),
                menu,
            )])],
        )
    }

    fn edit_borders(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut PageBorderDialog)) {
        self.edit_page_dialog(cx, |state| {
            state.commit();
            if let PageDialog::Borders(dialog) = &mut state.dialog {
                edit(dialog);
            }
        });
    }

    fn borders_body(&self, dialog: &PageBorderDialog, cx: &mut Context<Self>) -> AnyElement {
        let setting = dialog.setting();
        let sides = [
            ("border-top", "setup.margin.top", 0usize),
            ("border-bottom", "setup.margin.bottom", 1),
            ("border-left", "setup.margin.left", 2),
            ("border-right", "setup.margin.right", 3),
        ];
        let preview_color = if dialog.color == "auto" {
            gpui::rgb(0x000000)
        } else {
            gpui::rgb(u32::from_str_radix(&dialog.color, 16).unwrap_or(0))
        };
        let width = dialog.width;
        let stroke = move |on: bool| {
            if on {
                px(width.clamp(1.0, 4.0))
            } else {
                px(0.0)
            }
        };
        let settings: Vec<_> = [
            (BorderSetting::None, "setup.borders.none"),
            (BorderSetting::Box, "setup.borders.box"),
            (BorderSetting::Custom, "setup.borders.custom"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (entry, label))| {
            chip(("border-setting", index), t(label), setting == entry, true).on_click(cx.listener(
                move |view, _, _, cx| view.edit_borders(cx, |dialog| dialog.set_setting(entry)),
            ))
        })
        .collect();
        let styles: Vec<_> = PAGE_BORDER_STYLES
            .into_iter()
            .enumerate()
            .map(|(index, (value, label))| {
                chip(
                    ("border-style", index),
                    t(label),
                    dialog.style == value,
                    true,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.edit_borders(cx, |dialog| dialog.style = value.to_string())
                }))
            })
            .collect();
        let colors: Vec<_> = std::iter::once(("auto", "setup.borders.auto"))
            .chain(PAGE_COLORS.into_iter().skip(7))
            .enumerate()
            .map(|(index, (value, label))| {
                div()
                    .id(("border-color", index))
                    .size(px(18.0))
                    .rounded_sm()
                    .border_1()
                    .border_color(if dialog.color == value {
                        theme::accent()
                    } else {
                        theme::border()
                    })
                    .bg(if value == "auto" {
                        gpui::rgb(0x000000)
                    } else {
                        gpui::rgb(u32::from_str_radix(value, 16).unwrap_or(0))
                    })
                    .cursor_pointer()
                    .tooltip(Tooltip::text(t(label)))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_borders(cx, |dialog| dialog.color = value.to_string())
                    }))
            })
            .collect();
        let widths: Vec<_> = PAGE_BORDER_WIDTHS
            .into_iter()
            .enumerate()
            .map(|(index, entry)| {
                chip(
                    ("border-width", index),
                    format!("{entry} pt"),
                    (dialog.width - entry).abs() < 0.01,
                    true,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.edit_borders(cx, |dialog| dialog.width = entry)
                }))
            })
            .collect();
        let offsets: Vec<_> = [
            ("text", "setup.borders.from_text"),
            ("page", "setup.borders.from_page"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (value, label))| {
            chip(
                ("border-offset", index),
                t(label),
                dialog.offset_from == value,
                true,
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                view.edit_borders(cx, |dialog| dialog.offset_from = value.to_string())
            }))
        })
        .collect();
        let side_chips: Vec<_> = sides
            .into_iter()
            .map(|(id, label, index)| {
                chip(id, t(label), dialog.sides[index], true).on_click(cx.listener(
                    move |view, _, _, cx| {
                        view.edit_borders(cx, |dialog| dialog.sides[index] = !dialog.sides[index])
                    },
                ))
            })
            .collect();
        div()
            .flex()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .flex_1()
                    .child(line("setup.borders.setting").children(settings))
                    .child(line("setup.borders.style").children(styles))
                    .child(line("setup.borders.color").children(colors))
                    .child(line("setup.borders.width").children(widths))
                    .child(line("setup.borders.offset").children(offsets)),
            )
            .child(
                // プレビュー: the sides can be turned on and off one by one.
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(t("setup.borders.preview"))
                    .child(
                        div()
                            .w(px(90.0))
                            .h(px(120.0))
                            .bg(gpui::rgb(0xFFFFFF))
                            .border_color(preview_color)
                            .border_t(stroke(dialog.sides[0]))
                            .border_b(stroke(dialog.sides[1]))
                            .border_l(stroke(dialog.sides[2]))
                            .border_r(stroke(dialog.sides[3])),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .w(px(120.0))
                            .children(side_chips),
                    ),
            )
            .into_any_element()
    }

    fn edit_columns(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut ColumnsDialog)) {
        self.edit_page_dialog(cx, |state| {
            state.commit();
            if let PageDialog::Columns(dialog) = &mut state.dialog {
                edit(dialog);
            }
        });
    }

    fn columns_body(&self, dialog: &ColumnsDialog, cx: &mut Context<Self>) -> AnyElement {
        let widths = dialog.widths();
        let equal = dialog.columns.equal_width || dialog.columns.columns.is_empty();
        let presets = [
            (ColumnPreset::One, "setup.columns.one"),
            (ColumnPreset::Two, "setup.columns.two"),
            (ColumnPreset::Three, "setup.columns.three"),
            (ColumnPreset::Left, "setup.columns.left"),
            (ColumnPreset::Right, "setup.columns.right"),
        ];
        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(
                line("setup.columns.kind").children(presets.into_iter().enumerate().map(
                    |(index, (preset, label))| {
                        chip(("columns-kind", index), t(label), false, true).on_click(cx.listener(
                            move |view, _, _, cx| {
                                view.edit_columns(cx, |dialog| dialog.apply_preset(preset))
                            },
                        ))
                    },
                )),
            )
            .child(line("setup.columns.count").child(self.number_field(
                Key::ColumnCount,
                "setup.unit.columns",
                true,
                cx,
            )))
            .child(heading("setup.columns.widths"))
            .children(widths.iter().enumerate().map(|(index, _)| {
                let last = index + 1 == widths.len();
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .w(px(40.0))
                            .text_color(theme::text_dim())
                            .child(format!("{}", index + 1)),
                    )
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("setup.columns.width")),
                    )
                    .child(self.number_field(
                        Key::ColumnWidth(index),
                        "setup.unit.mm",
                        !equal || index == 0,
                        cx,
                    ))
                    .child(div().w(px(12.0)))
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("setup.columns.space")),
                    )
                    .child(self.number_field(
                        Key::ColumnSpace(index),
                        "setup.unit.mm",
                        !last && (!equal || index == 0),
                        cx,
                    ))
            }))
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        check("columns-equal", t("setup.columns.equal"), equal).on_click(
                            cx.listener(move |view, _, _, cx| {
                                view.edit_columns(cx, |dialog| dialog.set_equal(!equal))
                            }),
                        ),
                    )
                    .child(
                        check(
                            "columns-line",
                            t("setup.columns.line"),
                            dialog.columns.separator,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_columns(cx, |dialog| {
                                dialog.columns.separator =
                                    !dialog.columns.separator && dialog.columns.count > 1
                            })
                        })),
                    ),
            )
            .into_any_element()
    }

    fn edit_page_numbers(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut PageNumberDialog),
    ) {
        self.edit_page_dialog(cx, |state| {
            state.commit();
            if let PageDialog::PageNumbers(dialog) = &mut state.dialog {
                edit(dialog);
            }
        });
    }

    fn page_numbers_body(&self, dialog: &PageNumberDialog, cx: &mut Context<Self>) -> AnyElement {
        let numbering = &dialog.numbering;
        let chapter = numbering.chapter_style;
        let continues = numbering.start.is_none();
        div()
            .flex()
            .flex_col()
            .gap_1p5()
            .child(
                line("setup.numbers.format").children(
                    PAGE_NUMBER_FORMATS
                        .into_iter()
                        .enumerate()
                        .map(|(index, (value, label))| {
                            chip(
                                ("number-format", index),
                                label,
                                numbering.format == value,
                                true,
                            )
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    view.edit_page_numbers(cx, |dialog| {
                                        dialog.numbering.format = value.to_string()
                                    })
                                },
                            ))
                        }),
                ),
            )
            .child(
                check("chapter", t("setup.numbers.chapter"), chapter.is_some()).on_click(
                    cx.listener(move |view, _, _, cx| {
                        view.edit_page_numbers(cx, |dialog| {
                            dialog.set_chapter(if chapter.is_some() { None } else { Some(1) })
                        })
                    }),
                ),
            )
            .when(chapter.is_some(), |body| {
                body.child(
                    line("setup.numbers.chapter_style").children((1u8..=9).map(|level| {
                        chip(
                            ("chapter-level", level as usize),
                            i18n::format("setup.numbers.heading_level", &[("n", &level)]),
                            chapter == Some(level),
                            true,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.edit_page_numbers(cx, |dialog| dialog.set_chapter(Some(level)))
                        }))
                    })),
                )
                .child(
                    line("setup.numbers.separator").children(
                        CHAPTER_SEPARATORS.into_iter().enumerate().map(
                            |(index, (value, label))| {
                                chip(
                                    ("chapter-separator", index),
                                    label,
                                    numbering.chapter_separator.as_deref() == Some(value),
                                    true,
                                )
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        view.edit_page_numbers(cx, |dialog| {
                                            dialog.numbering.chapter_separator =
                                                Some(value.to_string())
                                        })
                                    },
                                ))
                            },
                        ),
                    ),
                )
            })
            .child(heading("setup.numbers.numbering"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        chip(
                            "numbers-continue",
                            t("setup.numbers.continue"),
                            continues,
                            true,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_page_numbers(cx, |dialog| dialog.set_continue(true))
                        })),
                    )
                    .child(
                        chip("numbers-start", t("setup.numbers.start"), !continues, true).on_click(
                            cx.listener(|view, _, _, cx| {
                                view.edit_page_numbers(cx, |dialog| dialog.set_continue(false))
                            }),
                        ),
                    )
                    .child(self.number_field(Key::PageStart, "setup.unit.none", !continues, cx)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{PageDialog, SetupAction};
    use crate::ribbon::Action;
    use jade_office_model::word::page_setup::{
        mm_to_points, ApplyTo, ColumnPreset, LineNumbersPreset, PageBreak, SetupTab, WordPageSetup,
    };

    fn open_contract(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// ページ設定 opens on the caret's section; a margin typed into its
    /// field, the gutter stepped and 見開きページ chosen land on OK as one
    /// undo step, and Escape leaves the page as it was.
    #[gpui::test]
    fn the_page_setup_dialog_types_margins_and_applies_them(cx: &mut gpui::TestAppContext) {
        use crate::Undo;
        let (view, cx) = open_contract(cx);
        let before = view.read_with(cx, |view, _| view.document.section_layout());
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Setup(SetupAction::Open(SetupTab::Margins)), cx);
            let state = view.page_dialog.as_mut().unwrap();
            let PageDialog::Setup(dialog) = &state.dialog else {
                panic!("the ページ設定 dialog");
            };
            assert_eq!(dialog.apply_to, dialog.apply_choices[0]);
            state.typing = Some((
                super::Key::Setup(jade_office_model::word::page_setup::Field::MarginLeft),
                String::new(),
                false,
            ));
        });
        cx.simulate_keystrokes("3 2 . 5");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            let state = view.page_dialog.as_mut().unwrap();
            assert!(state.typing.is_none(), "Enter ends the typing first");
            state.step(
                super::Key::Setup(jade_office_model::word::page_setup::Field::Gutter),
                true,
            );
            if let PageDialog::Setup(dialog) = &mut state.dialog {
                dialog
                    .set_multiple_pages(jade_office_model::word::page_setup::MultiplePages::Mirror);
                dialog.apply_to = ApplyTo::WholeDocument;
            }
            let _ = cx;
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.page_dialog.is_none());
            let layout = view.document.section_layout();
            assert!((layout.margin_left - mm_to_points(32.5)).abs() < 0.05);
            assert!((layout.gutter - mm_to_points(1.0)).abs() < 0.05);
            assert!(layout.mirror_margins);
        });
        cx.dispatch_action(Undo);
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.section_layout(), before)
        });

        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Setup(SetupAction::Open(SetupTab::Grid)), cx);
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| {
            assert!(view.page_dialog.is_none());
            assert_eq!(view.document.section_layout(), before);
        });
    }

    /// The レイアウト tab's menus: 段組み's narrow left column, 行番号, 縦書き
    /// and a page break at the caret.
    #[gpui::test]
    fn the_layout_menus_set_columns_line_numbers_and_breaks(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.document.place_caret(view.insert_at.0, 0);
            view.run_ribbon(Action::Setup(SetupAction::Columns(ColumnPreset::Left)), cx);
            let layout = view.document.section_layout();
            assert_eq!(layout.columns.count, 2);
            assert!(!layout.columns.equal_width);
            assert!(layout.columns.columns[0].width < layout.columns.columns[1].width);

            view.run_ribbon(
                Action::Setup(SetupAction::LineNumbers(
                    LineNumbersPreset::RestartEachSection,
                )),
                cx,
            );
            assert_eq!(
                view.document.line_numbers_preset(),
                LineNumbersPreset::RestartEachSection
            );

            view.run_ribbon(Action::Setup(SetupAction::Vertical(true)), cx);
            assert!(view.document.vertical_text());
            view.run_ribbon(Action::Setup(SetupAction::Vertical(false)), cx);
            assert!(!view.document.vertical_text());

            let pages = view.document.page_count();
            view.run_ribbon(Action::Setup(SetupAction::Break(PageBreak::Page)), cx);
            assert!(
                view.document.page_count() > pages,
                "the page break starts a page"
            );
        });
    }

    /// ⌘↩ puts a page break at the caret, as in Word for Mac, rather than a
    /// new paragraph.
    #[gpui::test]
    fn command_return_breaks_the_page(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        let (paragraph, paragraphs) = view.update(cx, |view, _| {
            let (paragraph, _) = view.insert_at;
            view.document.place_caret(paragraph, 0);
            (paragraph, view.document.paragraph_count())
        });
        cx.simulate_keystrokes("cmd-enter");
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.paragraph_count(), paragraphs);
            assert_eq!(view.insert_at.0, paragraph);
            let text = view.document.paragraph_text(view.insert_at.0).unwrap();
            assert!(text.contains('\u{000C}'), "{text:?}");
        });
    }
}

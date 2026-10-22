//! The browser shell's copy of the native app's `word_characters`: Word's
//! character formatting and Japanese text tools beyond the ribbon's
//! toggles, over the shared commands in `jade-office-model`
//! (`word::font`, `word::insert`, `word::asian`, `word::clipboard`):
//!
//! - the フォント dialog (⌘D) with its フォント and 詳細設定 tabs,
//! - the font group's third row: 文字種の変換, 下線のスタイル, 文字の網かけ,
//!   囲み線, ルビ, 囲い文字 and 拡張書式,
//! - the テーマの色 palette and その他の色 for the font colour,
//! - 記号と特殊文字, 日付と時刻, 形式を選択して貼り付け and 文字体裁.
//!
//! One dialog is open at a time, over the window; while it is, it takes the
//! keyboard — typing goes into its field, not the document.

mod color;
mod enclose;
mod font;
mod insert;
mod paste;
mod ribbon;
mod ruby;
mod typography;

use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, ClickEvent, Context, ElementId,
    MouseButton, SharedString, Window,
};
use jade_web::text::TextEdit;
use jade_web::word::font::{
    AsianLayout, FontDialog, HeadlessChangeCaseMode, HeadlessRunFormat, WordCharacters,
};
use jade_web::word::typography::WordTypography;

use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

pub use paste::{open as open_paste_special, paste_with};
pub use ribbon::{font_color_menu, font_family_menu, font_row, insert_group, paste_options};

/// The lists the font group's third row drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharPopup {
    ChangeCase,
    Underline,
    AsianLayout,
    /// The ▾ beside 貼り付け.
    PasteOptions,
}

/// What the third row, the colour menus and the dialogs' launchers do.
#[derive(Clone, Debug, PartialEq)]
pub enum CharAction {
    /// フォント… (⌘D).
    OpenFontDialog,
    /// A 下線のスタイル entry.
    Underline(&'static str),
    /// 下線の色.
    UnderlineColor(String),
    /// A テーマの色 swatch, by row and column.
    ThemeColor(usize, usize),
    /// その他の色… for the text colour.
    MoreFontColors,
    /// 文字の網かけ.
    Shading,
    /// 囲み線.
    Border,
    /// 文字種の変換.
    ChangeCase(HeadlessChangeCaseMode),
    /// 拡張書式's 縦中横, 組み文字, 割注 and 解除.
    AsianLayout(AsianLayout),
    /// 文字の拡大/縮小.
    Scale(f32),
    /// 体裁…: the Japanese typography dialog.
    OpenTypography,
    /// 記号と特殊文字….
    OpenSymbols,
    /// 日付と時刻….
    OpenDateTime,
    /// ルビ….
    OpenRuby,
    /// 囲い文字….
    OpenEnclose,
    /// 貼り付けのオプション: the clipboard pasted in one of its modes.
    Paste(jade_web::word::clipboard::PasteMode),
    /// 形式を選択して貼り付け….
    OpenPasteSpecial,
}

/// The open dialog.
pub enum Dialog {
    Font(Box<FontDialog>),
    /// その他の色: the picker, and what it colours.
    Color(color::ColorDialog),
    /// 体裁: the paragraph's and the document's Japanese typography.
    Typography(typography::TypographyState),
    /// 記号と特殊文字.
    Symbol(insert::SymbolDialog),
    /// 日付と時刻.
    DateTime(insert::DateTimeDialog),
    /// ルビ.
    Ruby(ruby::RubyState),
    /// 囲い文字.
    Enclose(enclose::EncloseDialog),
    /// 形式を選択して貼り付け.
    PasteSpecial(paste::PasteSpecial),
}

/// The lists a dialog drops over itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogList {
    JapaneseFont,
    LatinFont,
    ComplexFont,
    Size,
    FontColor,
    Underline,
    UnderlineColor,
    Emphasis,
    Scale,
    Ligatures,
    NumberSpacing,
    NumberForm,
    StylisticSet,
    Glow,
    TextOutline,
}

/// The view's share of this: the dialog, its open list and its field.
pub struct Characters {
    pub dialog: Option<Dialog>,
    pub list: Option<DialogList>,
    /// Where typing goes while a dialog is open: a font list's filter, a
    /// colour's hex code.
    pub field: TextEdit,
    /// 記号と特殊文字's 最近使用した記号, most recent first.
    pub recent_symbols: Vec<char>,
}

impl Default for Characters {
    fn default() -> Self {
        Self {
            dialog: None,
            list: None,
            field: TextEdit::new(String::new()),
            recent_symbols: Vec::new(),
        }
    }
}

impl Characters {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// The field typing goes into while a dialog is open.
    pub fn text_field(&self) -> Option<&TextEdit> {
        self.dialog.as_ref().map(|_| &self.field)
    }

    pub fn text_field_mut(&mut self) -> Option<&mut TextEdit> {
        match self.dialog {
            Some(_) => Some(&mut self.field),
            None => None,
        }
    }

    fn open(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
        self.list = None;
        self.field = TextEdit::new(String::new());
    }

    pub fn close(&mut self) {
        self.dialog = None;
        self.list = None;
        self.field = TextEdit::new(String::new());
    }

    /// The system's families, Japanese and Latin, for the font lists.
    pub fn font_lists(&mut self, cx: &gpui::App) -> (Vec<String>, Vec<String>) {
        installed_font_lists(cx)
    }
}

/// The system's families, Japanese and Latin, read once per thread.
pub fn installed_font_lists(cx: &gpui::App) -> (Vec<String>, Vec<String>) {
    thread_local! {
        static LISTS: std::cell::RefCell<Option<(Vec<String>, Vec<String>)>> =
            const { std::cell::RefCell::new(None) };
    }
    LISTS.with(|lists| {
        lists
            .borrow_mut()
            .get_or_insert_with(|| {
                jade_web::word::font::font_lists(&cx.text_system().all_font_names())
            })
            .clone()
    })
}

/// The document's theme colours, by the theme part's slot names.
pub fn document_theme_colors(
    document: &jade_web::editing::EditableDocument,
) -> Vec<(String, String)> {
    use jade_web::editing::WordSession;
    let Some(theme) = document.word_document().document_theme() else {
        return Vec::new();
    };
    [
        ("dk2", theme.dark2),
        ("lt2", theme.light2),
        ("accent1", theme.accent1),
        ("accent2", theme.accent2),
        ("accent3", theme.accent3),
        ("accent4", theme.accent4),
        ("accent5", theme.accent5),
        ("accent6", theme.accent6),
    ]
    .into_iter()
    .map(|(slot, color)| (slot.to_string(), color))
    .collect()
}

/// The open document's theme colours.
pub fn theme_colors(view: &DocumentView) -> Vec<(String, String)> {
    view.word_ref()
        .map(|document| document_theme_colors(document))
        .unwrap_or_default()
}

/// Runs a third-row or colour-menu action.
pub fn run(view: &mut DocumentView, action: CharAction, cx: &mut Context<DocumentView>) {
    match action {
        CharAction::OpenFontDialog => return open_font_dialog(view, cx),
        CharAction::OpenTypography => return open_typography(view, cx),
        CharAction::OpenSymbols => return insert::open_symbols(view, cx),
        CharAction::OpenDateTime => return insert::open_date_time(view, cx),
        CharAction::OpenRuby => return ruby::open(view, cx),
        CharAction::OpenEnclose => return enclose::open(view, cx),
        CharAction::Paste(mode) => return paste::paste_with(view, mode, cx),
        CharAction::OpenPasteSpecial => return paste::open(view, cx),
        CharAction::MoreFontColors => {
            let current = view
                .word_ref()
                .and_then(|document| WordCharacters::character_format(document).color);
            view.characters
                .open(Dialog::Color(color::ColorDialog::for_font(current)));
            return cx.notify();
        }
        _ => {}
    }
    let palette = jade_web::word::font::theme_palette(&theme_colors(view));
    let Some(document) = view.word_mut() else {
        return;
    };
    let done = match action {
        CharAction::Underline(kind) => document.set_underline_style(kind),
        CharAction::UnderlineColor(color) => document.set_underline_color(&color),
        CharAction::ThemeColor(row, column) => {
            match palette.get(row).and_then(|row| row.get(column)) {
                Some(swatch) => document.set_theme_font_color(swatch),
                None => Ok(()),
            }
        }
        CharAction::Shading => document.toggle_character_shading(),
        CharAction::Border => document.toggle_character_border(),
        CharAction::ChangeCase(mode) => document.change_case(mode),
        CharAction::AsianLayout(layout) => document.set_asian_layout(layout),
        CharAction::Scale(percent) => document.set_character_scale(percent),
        _ => Ok(()),
    };
    view.after_character_edit(done, cx);
}

/// Opens the フォント dialog on the selection, or the word at the caret.
pub fn open_font_dialog(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(dialog) = view.word_ref().map(WordCharacters::font_dialog) else {
        return;
    };
    view.characters.open(Dialog::Font(Box::new(dialog)));
    cx.notify();
}

/// Opens 体裁 on the selected paragraphs.
pub fn open_typography(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(dialog) = view.word_ref().map(WordTypography::typography_dialog) else {
        return;
    };
    view.characters
        .open(Dialog::Typography(typography::TypographyState {
            dialog: Box::new(dialog),
            editing: None,
        }));
    cx.notify();
}

/// OK: what the dialog says lands on the document, as one undo step.
pub fn confirm(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    typography::commit_field(view);
    ruby::commit_field(view);
    let typed = std::mem::replace(&mut view.characters.field, TextEdit::new(String::new()));
    let Some(mut dialog) = view.characters.dialog.take() else {
        return;
    };
    // A code typed into 文字コード wins over the grid.
    if let Dialog::Symbol(symbol) = &mut dialog {
        if let Some(code) = jade_web::word::insert::character_from_code(&typed.text) {
            symbol.chosen = Some(code);
        }
    }
    view.characters.close();
    let done = match dialog {
        Dialog::Font(dialog) => {
            view.word_edit_result(|document| WordCharacters::apply_font_dialog(document, &dialog))
        }
        Dialog::Typography(state) => view
            .word_edit_result(|document| WordTypography::apply_typography(document, &state.dialog)),
        Dialog::Symbol(dialog) => return insert::confirm_symbol(view, dialog, cx),
        Dialog::DateTime(dialog) => return insert::confirm_date_time(view, dialog, cx),
        Dialog::Ruby(state) => return ruby::confirm(view, state, cx),
        Dialog::Enclose(dialog) => return enclose::confirm(view, dialog, typed.text, cx),
        Dialog::PasteSpecial(dialog) => return paste::confirm(view, dialog, cx),
        Dialog::Color(dialog) => match dialog.back {
            // From the フォント dialog: back to it, with the colour chosen.
            Some(mut font) => {
                font.format.color = Some(dialog.color.to_ascii_uppercase());
                font.format.theme_color = None;
                view.characters.open(Dialog::Font(font));
                return cx.notify();
            }
            None => view.word_edit_result(|document| {
                WordCharacters::apply_run_format(
                    document,
                    HeadlessRunFormat {
                        color: Some(dialog.hex()),
                        ..Default::default()
                    },
                )
            }),
        },
    };
    view.after_character_edit(done, cx);
}

/// Escape or キャンセル: nothing changes. A colour picked from the フォント
/// dialog goes back to it.
pub fn cancel(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.characters.list.take().is_some() {
        return cx.notify();
    }
    if let Some(Dialog::Color(dialog)) = view.characters.dialog.take() {
        if let Some(font) = dialog.back {
            view.characters.open(Dialog::Font(font));
        }
    }
    if !matches!(view.characters.dialog, Some(Dialog::Font(_))) {
        view.characters.close();
    }
    cx.notify();
}

/// A key while a dialog is open; `true` when it was taken.
pub fn handle_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    if view.characters.field.marked.is_some() {
        return false;
    }
    match key {
        "escape" => cancel(view, cx),
        "enter" | "return" => {
            // In a font list, Enter takes the first font the filter left.
            if let Some(list) = view.characters.list {
                font::pick_filtered(view, list, cx);
            } else {
                confirm(view, cx);
            }
        }
        "backspace" => {
            view.characters.field.backspace();
            cx.notify();
        }
        "delete" => {
            view.characters.field.delete();
            cx.notify();
        }
        "left" => {
            view.characters.field.step(false);
            cx.notify();
        }
        "right" => {
            view.characters.field.step(true);
            cx.notify();
        }
        // Typing reaches the field through the input handler; the
        // document behind the dialog gets nothing.
        _ => return false,
    }
    true
}

/// The open dialog, over the window.
pub fn overlay(
    view: &mut DocumentView,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    let body = match view.characters.dialog.as_ref()? {
        Dialog::Font(_) => font::body(view, cx),
        Dialog::Color(_) => color::body(view, cx),
        Dialog::Typography(_) => typography::body(view, cx),
        Dialog::Symbol(_) => insert::symbol_body(view, cx),
        Dialog::DateTime(_) => insert::date_time_body(view, cx),
        Dialog::Ruby(_) => ruby::body(view, cx),
        Dialog::Enclose(_) => enclose::body(view, cx),
        Dialog::PasteSpecial(_) => paste::body(view, cx),
    };
    let title = match view.characters.dialog.as_ref()? {
        Dialog::Font(_) => i18n::text("chars.font.title"),
        Dialog::Color(_) => i18n::text("chars.color.title"),
        Dialog::Typography(_) => i18n::text("chars.typography.title"),
        Dialog::Symbol(_) => i18n::text("chars.symbol.title"),
        Dialog::DateTime(_) => i18n::text("chars.date.title"),
        Dialog::Ruby(_) => i18n::text("chars.ruby.title"),
        Dialog::Enclose(_) => i18n::text("chars.enclose.title"),
        Dialog::PasteSpecial(_) => i18n::text("chars.paste.title"),
    };
    let viewport = window.viewport_size();
    Some(
        deferred(
            anchored().position(point(px(0.0), px(0.0))).child(
                div()
                    .id("char-dialog-backdrop")
                    .w(viewport.width)
                    .h(viewport.height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui::rgba(0x00000040))
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .id("char-dialog")
                            .max_h(viewport.height - px(40.0))
                            .overflow_y_scroll()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .rounded_lg()
                            .border_1()
                            .border_color(theme::border())
                            .bg(theme::panel())
                            .shadow_lg()
                            .font_family(crate::UI_FONT)
                            .text_xs()
                            .text_color(theme::text())
                            .child(div().text_sm().child(title))
                            .child(body)
                            .child(buttons(cx)),
                    ),
            ),
        )
        .with_priority(3)
        .into_any_element(),
    )
}

/// キャンセル and OK.
fn buttons(cx: &mut Context<DocumentView>) -> AnyElement {
    div()
        .flex()
        .justify_end()
        .gap_2()
        .child(
            button("char-cancel", i18n::text("format.cancel"), false)
                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| cancel(view, cx))),
        )
        .child(
            button("char-ok", i18n::text("format.ok"), true)
                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| confirm(view, cx))),
        )
        .into_any_element()
}

// ---- the dialogs' controls ----------------------------------------------

/// A push button; `primary` is the default one.
pub(crate) fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    primary: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(if primary {
            theme::accent()
        } else {
            theme::border()
        })
        .when(primary, |button| button.bg(theme::selected()))
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .child(label.into())
}

/// A check box with its label.
pub(crate) fn check(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
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
                .size(px(13.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .border_1()
                .border_color(if on { theme::accent() } else { theme::border() })
                .when(on, |box_| {
                    box_.bg(theme::accent()).text_color(gpui::white())
                })
                .text_size(px(10.0))
                .child(if on { "✓" } else { "" }),
        )
        .child(label.into())
}

/// A caption over a control.
pub(crate) fn caption(text: impl Into<SharedString>) -> gpui::Div {
    div().text_color(theme::dim()).child(text.into())
}

/// A field-like button showing a value, which opens `list`.
pub(crate) fn select(
    id: impl Into<ElementId>,
    value: impl Into<SharedString>,
    width: f32,
    list: DialogList,
    open: bool,
    cx: &mut Context<DocumentView>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .w(px(width))
        .h(px(22.0))
        .px_2()
        .rounded_sm()
        .border_1()
        .border_color(if open {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(theme::canvas())
        .cursor_pointer()
        .overflow_hidden()
        .whitespace_nowrap()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .child(value.into()),
        )
        .child("▾")
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
            let characters = &mut view.characters;
            characters.list = (characters.list != Some(list)).then_some(list);
            characters.field = TextEdit::new(String::new());
            cx.notify();
        }))
}

/// A select with its list under it while open.
pub(crate) fn with_list(trigger: impl IntoElement, list: Option<AnyElement>) -> AnyElement {
    div()
        .relative()
        .flex_none()
        .child(trigger)
        .children(list.map(|list| {
            div().absolute().top_full().left_0().child(
                deferred(anchored().snap_to_window_with_margin(px(8.0)).child(list))
                    .with_priority(4),
            )
        }))
        .into_any_element()
}

/// The panel a dialog's list is drawn on.
pub(crate) fn list_panel(id: &'static str, width: f32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .occlude()
        .mt_1()
        .p_1()
        .w(px(width))
        .max_h(px(300.0))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .rounded_md()
        .bg(theme::panel())
        .border_1()
        .border_color(theme::border())
        .shadow_lg()
        .font_family(crate::UI_FONT)
        .text_xs()
        .text_color(theme::text())
}

/// One entry of a dialog's list.
pub(crate) fn list_row(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .h(px(22.0))
        .px_1()
        .rounded_sm()
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|row| row.bg(theme::hovered()))
        .when(checked, |row| row.bg(theme::selected()))
        .child(label.into())
}

/// A −/value/+ stepper.
pub(crate) fn stepper(
    id: &'static str,
    value: String,
    enabled: bool,
    cx: &mut Context<DocumentView>,
    step: impl Fn(&mut DocumentView, bool) + Clone + 'static,
) -> AnyElement {
    let less = step.clone();
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(
            div()
                .id((id, 0usize))
                .px_1()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .when(enabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(|button| button.bg(theme::hovered()))
                        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                            less(view, false);
                            cx.notify();
                        }))
                })
                .child("−"),
        )
        .child(
            div()
                .min_w(px(52.0))
                .px_1()
                .border_1()
                .border_color(theme::border())
                .bg(theme::canvas())
                .when(!enabled, |field| field.text_color(theme::faint()))
                .child(value),
        )
        .child(
            div()
                .id((id, 1usize))
                .px_1()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .when(enabled, |button| {
                    button
                        .cursor_pointer()
                        .hover(|button| button.bg(theme::hovered()))
                        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                            step(view, true);
                            cx.notify();
                        }))
                })
                .child("+"),
        )
        .into_any_element()
}

/// A colour square.
pub(crate) fn swatch(id: impl Into<ElementId>, color: &str, on: bool) -> gpui::Stateful<gpui::Div> {
    let rgb = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0);
    div()
        .id(id)
        .size(px(16.0))
        .rounded_sm()
        .bg(gpui::rgb(rgb))
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .cursor_pointer()
        .hover(|square| square.border_color(theme::accent()))
}

/// Points with Word's decimals: `10.5`, `12`.
pub(crate) fn points(value: f32) -> String {
    if (value - value.round()).abs() < 0.01 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

impl DocumentView {
    /// The Word document, when this view is one.
    pub(crate) fn word_ref(&self) -> Option<&jade_web::editing::WordDoc> {
        match &self.document {
            super::Document::Pages { document, .. } => Some(document),
            _ => None,
        }
    }

    pub(crate) fn word_mut(&mut self) -> Option<&mut jade_web::editing::WordDoc> {
        match &mut self.document {
            super::Document::Pages { document, .. } => Some(document),
            _ => None,
        }
    }

    /// Runs `edit` on the Word document, or says there is none.
    pub(crate) fn word_edit_result(
        &mut self,
        edit: impl FnOnce(&mut jade_web::editing::WordDoc) -> Result<(), String>,
    ) -> Result<(), String> {
        match self.word_mut() {
            Some(document) => edit(document),
            None => Err("no Word document".to_string()),
        }
    }

    /// After a character command: the pages repainted, or why nothing
    /// changed.
    pub(crate) fn after_character_edit(
        &mut self,
        done: Result<(), String>,
        cx: &mut Context<DocumentView>,
    ) {
        self.refresh_repainted();
        self.reveal_caret();
        self.word_result(done, cx);
    }
}

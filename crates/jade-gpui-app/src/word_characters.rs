//! Word's character formatting and Japanese text tools beyond the ribbon's
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
use jade_office_model::text::TextEdit;
use jade_office_model::word::font::{
    AsianLayout, FontDialog, HeadlessChangeCaseMode, HeadlessRunFormat, WordCharacters,
};
use jade_office_model::word::typography::WordTypography;

use crate::{i18n, theme, DocumentView};

pub use insert::{local_now, open_date_time, open_symbols, renderer_clock};
pub use paste::{open as open_paste_special, paste_with};
pub use ribbon::{font_color_menu, font_family_menu, font_row, insert_group, paste_options};

/// The lists the font group's third row drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharPopup {
    ChangeCase,
    Underline,
    AsianLayout,
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
    /// A 貼り付けのオプション entry.
    Paste(jade_office_model::word::clipboard::PasteMode),
    /// 形式を選択して貼り付け….
    OpenPasteSpecial,
    /// 記号と特殊文字….
    OpenSymbols,
    /// 日付と時刻….
    OpenDateTime,
    /// ルビ….
    OpenRuby,
    /// 囲い文字….
    OpenEnclose,
}

/// The open dialog.
pub enum Dialog {
    Font(Box<FontDialog>),
    /// その他の色: the picker, and what it colours.
    Color(color::ColorDialog),
    /// 体裁: the paragraph's and the document's Japanese typography.
    Typography(typography::TypographyState),
    /// 形式を選択して貼り付け.
    PasteSpecial(paste::PasteSpecial),
    /// 記号と特殊文字.
    Symbol(insert::SymbolDialog),
    /// 日付と時刻.
    DateTime(insert::DateTimeDialog),
    /// ルビ.
    Ruby(ruby::RubyState),
    /// 囲い文字.
    Enclose(enclose::EncloseDialog),
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
                jade_office_model::word::font::font_lists(&cx.text_system().all_font_names())
            })
            .clone()
    })
}

/// The document's theme colours, by the theme part's slot names.
pub fn document_theme_colors(
    document: &jade_docx_native_render::NativeDocument,
) -> Vec<(String, String)> {
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
    view.ribbon.theme_colors.clone()
}

/// Runs a third-row or colour-menu action.
pub fn run(view: &mut DocumentView, action: CharAction, cx: &mut Context<DocumentView>) {
    view.ribbon.popup = None;
    let document = &mut view.document;
    let done = match action {
        CharAction::OpenFontDialog => return open_font_dialog(view, cx),
        CharAction::OpenTypography => return open_typography(view, cx),
        CharAction::Paste(mode) => return paste::paste_with(view, mode, cx),
        CharAction::OpenPasteSpecial => return paste::open(view, cx),
        CharAction::OpenSymbols => return insert::open_symbols(view, cx),
        CharAction::OpenDateTime => return insert::open_date_time(view, cx),
        CharAction::OpenRuby => return ruby::open(view, cx),
        CharAction::OpenEnclose => return enclose::open(view, cx),
        CharAction::MoreFontColors => {
            let current = WordCharacters::character_format(&view.document).color;
            view.characters
                .open(Dialog::Color(color::ColorDialog::for_font(current)));
            return cx.notify();
        }
        CharAction::Underline(kind) => document.set_underline_style(kind),
        CharAction::UnderlineColor(color) => document.set_underline_color(&color),
        CharAction::ThemeColor(row, column) => {
            let palette = jade_office_model::word::font::theme_palette(&theme_colors(view));
            match palette.get(row).and_then(|row| row.get(column)) {
                Some(swatch) => view.document.set_theme_font_color(swatch),
                None => Ok(()),
            }
        }
        CharAction::Shading => document.toggle_character_shading(),
        CharAction::Border => document.toggle_character_border(),
        CharAction::ChangeCase(mode) => document.change_case(mode),
        CharAction::AsianLayout(layout) => document.set_asian_layout(layout),
        CharAction::Scale(percent) => document.set_character_scale(percent),
    };
    view.after_character_edit(done, cx);
}

/// Opens the フォント dialog on the selection, or the word at the caret.
pub fn open_font_dialog(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.document.selection().is_none() {
        view.document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    let dialog = WordCharacters::font_dialog(&view.document);
    view.characters.open(Dialog::Font(Box::new(dialog)));
    cx.notify();
}

/// Opens 体裁 on the selected paragraphs.
pub fn open_typography(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.document.selection().is_none() {
        view.document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    let dialog = WordTypography::typography_dialog(&view.document);
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
        if let Some(code) = jade_office_model::word::insert::character_from_code(&typed.text) {
            symbol.chosen = Some(code);
        }
    }
    view.characters.close();
    let done = match dialog {
        Dialog::Font(dialog) => WordCharacters::apply_font_dialog(&mut view.document, &dialog),
        Dialog::Typography(state) => {
            WordTypography::apply_typography(&mut view.document, &state.dialog)
        }
        Dialog::PasteSpecial(dialog) => return paste::confirm(view, dialog, cx),
        Dialog::Symbol(dialog) => return insert::confirm_symbol(view, dialog, cx),
        Dialog::DateTime(dialog) => return insert::confirm_date_time(view, dialog, cx),
        Dialog::Ruby(state) => return ruby::confirm(view, state, cx),
        Dialog::Enclose(dialog) => return enclose::confirm(view, dialog, typed.text, cx),
        Dialog::Color(dialog) => match dialog.back {
            // From the フォント dialog: back to it, with the colour chosen.
            Some(mut font) => {
                font.format.color = Some(dialog.color.to_ascii_uppercase());
                font.format.theme_color = None;
                view.characters.open(Dialog::Font(font));
                return cx.notify();
            }
            None => WordCharacters::apply_run_format(
                &mut view.document,
                HeadlessRunFormat {
                    color: Some(dialog.hex()),
                    ..Default::default()
                },
            ),
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
        Dialog::PasteSpecial(_) => paste::body(view, cx),
        Dialog::Symbol(_) => insert::symbol_body(view, cx),
        Dialog::DateTime(_) => insert::date_time_body(view, cx),
        Dialog::Ruby(_) => ruby::body(view, cx),
        Dialog::Enclose(_) => enclose::body(view, cx),
    };
    let title = match view.characters.dialog.as_ref()? {
        Dialog::Font(_) => i18n::text("chars.font.title"),
        Dialog::Color(_) => i18n::text("chars.color.title"),
        Dialog::Typography(_) => i18n::text("chars.typography.title"),
        Dialog::PasteSpecial(_) => i18n::text("chars.paste.title"),
        Dialog::Symbol(_) => i18n::text("chars.symbol.title"),
        Dialog::DateTime(_) => i18n::text("chars.date.title"),
        Dialog::Ruby(_) => i18n::text("chars.ruby.title"),
        Dialog::Enclose(_) => i18n::text("chars.enclose.title"),
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
                            .font_family(crate::UI_FONT_FAMILY)
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
    div().text_color(theme::text_dim()).child(text.into())
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
        .font_family(crate::UI_FONT_FAMILY)
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
                .when(!enabled, |field| field.text_color(theme::text_faint()))
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
    /// After a character command: the pages repainted, or why nothing
    /// changed.
    pub(crate) fn after_character_edit(
        &mut self,
        done: Result<(), String>,
        cx: &mut Context<DocumentView>,
    ) {
        match done {
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
}

/// At open: a document with `DATE` or `TIME` fields shows today's date and
/// time in them, as Word does, rather than the result it was saved with.
pub fn start_field_clock(document: &mut jade_docx_native_render::NativeDocument) {
    if !jade_office_model::word::insert::has_clock_fields(document.word_document()) {
        return;
    }
    document.set_field_clock(Some(renderer_clock(&local_now())));
    if let Err(error) = document.repaint_all() {
        eprintln!("repaint with the clock failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::{CharAction, Dialog};
    use crate::ribbon::Action;
    use jade_office_model::word::font::{AsianLayout, Effect, HeadlessChangeCaseMode};

    /// The contract, open with the keyboard on it and the first characters
    /// of the caret's paragraph selected.
    fn open_selected(
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
        view.update(cx, |view, _| select_start(view));
        (view, cx)
    }

    fn select_start(view: &mut crate::DocumentView) {
        let paragraph = view.insert_at.0;
        let text = view.current_paragraph_text();
        let end = text.char_indices().nth(3).map(|(index, _)| index).unwrap();
        view.document
            .select((paragraph, 0), (paragraph, end), 1)
            .unwrap();
        view.insert_at = (paragraph, end);
    }

    fn document_xml(view: &crate::DocumentView) -> String {
        let bytes = view.document.export_docx().unwrap();
        let package = jade_opc::OpcPackage::open(&bytes).unwrap();
        package.part_str("word/document.xml").unwrap().unwrap()
    }

    /// ⌘D opens the フォント dialog on the selection; what is changed there
    /// lands as one undo step on OK, and Escape leaves the text alone.
    #[gpui::test]
    fn the_font_dialog_applies_what_was_changed(cx: &mut gpui::TestAppContext) {
        use crate::{Editor, FontDialog, Undo};
        let (view, cx) = open_selected(cx);
        cx.dispatch_action(FontDialog);
        view.update(cx, |view, cx| {
            let Some(Dialog::Font(dialog)) = view.characters.dialog.as_mut() else {
                panic!("⌘D opens the dialog");
            };
            dialog.set_effect(Effect::DoubleStrike, true);
            dialog.set_effect(Effect::SmallCaps, true);
            dialog.format.east_asia_font = Some("游ゴシック".into());
            dialog.format.emphasis = Some("dot".into());
            dialog.step_spacing(1.0);
            super::confirm(view, cx);
            assert!(view.characters.dialog.is_none());
            let format = view.format_state().character;
            assert_eq!(format.double_strike, Some(true));
            assert_eq!(format.small_caps, Some(true));
            assert_eq!(format.emphasis.as_deref(), Some("dot"));
            assert_eq!(format.east_asia_font.as_deref(), Some("游ゴシック"));
            assert!(view.is_dirty());
        });
        let xml = view.read_with(cx, |view, _| document_xml(view));
        assert!(xml.contains("<w:dstrike/>"), "{xml}");
        assert!(xml.contains(r#"<w:em w:val="dot"/>"#));
        // One undo step takes it all back.
        cx.dispatch_action(Undo);
        view.update(cx, |view, _| {
            let format = view.format_state().character;
            assert_ne!(format.double_strike, Some(true));
            assert_eq!(format.emphasis, None);
        });
        // Typing while the dialog is open goes to the dialog, and Escape
        // closes it with nothing applied.
        cx.dispatch_action(FontDialog);
        let before = view.read_with(cx, |view, _| view.current_paragraph_text());
        cx.simulate_input("x");
        cx.simulate_keystrokes("escape");
        view.update(cx, |view, _| {
            assert!(view.characters.dialog.is_none());
            assert_eq!(view.current_paragraph_text(), before);
        });
    }

    /// The font group's third row: an underline style, a theme colour,
    /// shading, a border, 縦中横 and 半角, each reading back.
    #[gpui::test]
    fn the_third_row_formats_the_selection(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_selected(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::Characters(CharAction::Underline("wave")), cx);
            view.run_ribbon(Action::Characters(CharAction::ThemeColor(5, 4)), cx);
            view.run_ribbon(Action::Characters(CharAction::Shading), cx);
            view.run_ribbon(Action::Characters(CharAction::Border), cx);
            view.run_ribbon(
                Action::Characters(CharAction::AsianLayout(AsianLayout::Vertical {
                    compress: true,
                })),
                cx,
            );
            let format = view.format_state().character;
            assert_eq!(format.underline.as_deref(), Some("wave"));
            let theme = format.theme_color.expect("a theme colour");
            assert_eq!(theme.name, "accent1");
            assert_eq!(theme.shade.as_deref(), Some("80"));
            assert!(format.shading.is_some_and(|fill| fill != "none"));
            assert_eq!(format.border, Some(true));
            assert!(format.east_asian_layout.is_some_and(|layout| layout.vert));
            // The buttons turn off again.
            view.run_ribbon(Action::Characters(CharAction::Shading), cx);
            view.run_ribbon(Action::Characters(CharAction::Border), cx);
            let format = view.format_state().character;
            assert!(format.shading.is_none());
            assert_ne!(format.border, Some(true));
        });
        let xml = view.read_with(cx, |view, _| document_xml(view));
        assert!(xml.contains(r#"w:val="wave""#));
        assert!(xml.contains(r#"w:themeColor="accent1""#));
        assert!(xml.contains(r#"w:vertCompress="1""#));
    }

    /// 体裁: turning off 日本語と英字の間隔を自動調整する closes the gap
    /// between Japanese and Latin text on the page; the document's kinsoku
    /// level goes into settings.xml; one undo takes both back.
    #[gpui::test]
    fn typography_relays_the_paragraph_and_writes_settings(cx: &mut gpui::TestAppContext) {
        use crate::{TypographyDialog, Undo};
        let (view, cx) = open_selected(cx);
        let (paragraph, latin_x) = view.update(cx, |view, _| {
            let paragraph = view.insert_at.0;
            view.document
                .insert_text(paragraph, 0, "日本語English", 1)
                .unwrap();
            view.document
                .select((paragraph, 0), (paragraph, 3), 1)
                .unwrap();
            let x = view
                .document
                .place_caret(paragraph, "日本語E".len())
                .unwrap()
                .x;
            view.document
                .select((paragraph, 0), (paragraph, 3), 1)
                .unwrap();
            (paragraph, x)
        });
        cx.dispatch_action(TypographyDialog);
        view.update(cx, |view, cx| {
            let Some(Dialog::Typography(state)) = view.characters.dialog.as_mut() else {
                panic!("the dialog opens");
            };
            assert_eq!(state.dialog.paragraph.auto_space_de, Some(true));
            state.dialog.paragraph.auto_space_de = Some(false);
            state.dialog.set_level("strict");
            super::confirm(view, cx);
            let x = view
                .document
                .place_caret(paragraph, "日本語E".len())
                .unwrap()
                .x;
            assert!(x < latin_x - 0.5, "the gap closes: {x} vs {latin_x}");
        });
        let (xml, settings) = view.read_with(cx, |view, _| {
            let bytes = view.document.export_docx().unwrap();
            let package = jade_opc::OpcPackage::open(&bytes).unwrap();
            (
                package.part_str("word/document.xml").unwrap().unwrap(),
                package.part_str("word/settings.xml").unwrap().unwrap(),
            )
        });
        assert!(xml.contains(r#"<w:autoSpaceDE w:val="0"/>"#));
        assert!(settings.contains("<w:strictFirstAndLastChars/>"));
        cx.dispatch_action(Undo);
        view.update(cx, |view, _| {
            let x = view
                .document
                .place_caret(paragraph, "日本語E".len())
                .unwrap()
                .x;
            assert!((x - latin_x).abs() < 0.5);
            let bytes = view.document.export_docx().unwrap();
            let package = jade_opc::OpcPackage::open(&bytes).unwrap();
            let settings = package.part_str("word/settings.xml").unwrap().unwrap();
            assert!(!settings.contains("strictFirstAndLastChars"));
        });
    }

    /// A bold copy pasted three ways: 元の書式を保持 keeps it bold and red,
    /// 書式を結合 keeps the bold but not the colour, テキストのみ保持 keeps
    /// neither; 形式を選択して貼り付け's 書式なしテキスト is the last.
    #[gpui::test]
    fn paste_options_keep_merge_or_drop_formatting(cx: &mut gpui::TestAppContext) {
        use crate::{Copy, PasteMatchFormatting, PasteSpecial};
        use jade_office_model::word::clipboard::PasteMode;
        let (view, cx) = open_selected(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(
                Action::Toggle(jade_office_model::word::CharacterFormat::Bold),
                cx,
            );
            view.run_ribbon(Action::FontColor("C00000".into()), cx);
        });
        cx.dispatch_action(Copy);
        // The end of the next paragraph, as a caret.
        let paste_at = |view: &mut crate::DocumentView| {
            let paragraph = view.insert_at.0 + 1;
            let end = view.document.paragraph_text(paragraph).unwrap().len();
            view.document.set_selection_range(None);
            view.insert_at = (paragraph, end);
            view.document.place_caret(paragraph, end);
            (paragraph, end)
        };
        let style_after = |view: &crate::DocumentView, at: (usize, usize)| {
            view.document.run_style_at(at.0, at.1).unwrap_or_default()
        };
        let at = view.update(cx, |view, cx| {
            let at = paste_at(view);
            super::paste_with(view, PasteMode::KeepSource, cx);
            at
        });
        view.read_with(cx, |view, _| {
            let style = style_after(view, at);
            assert_eq!(style.bold, Some(true));
            assert!(style.color.is_some_and(|color| color.contains("C00000")));
        });
        let at = view.update(cx, |view, _| paste_at_end(view, 1));
        cx.dispatch_action(PasteMatchFormatting);
        view.read_with(cx, |view, _| {
            let style = style_after(view, at);
            assert_eq!(style.bold, Some(true));
            assert!(!style.color.is_some_and(|color| color.contains("C00000")));
        });
        let at = view.update(cx, |view, _| paste_at_end(view, 1));
        cx.dispatch_action(PasteSpecial);
        view.update(cx, |view, cx| {
            let Some(Dialog::PasteSpecial(dialog)) = view.characters.dialog.as_mut() else {
                panic!("⌃⌘V opens the dialog");
            };
            assert_eq!(
                dialog.formats.len(),
                5,
                "a copy made here offers every format"
            );
            dialog.chosen = dialog
                .formats
                .iter()
                .position(|format| format.mode() == PasteMode::TextOnly)
                .unwrap();
            super::confirm(view, cx);
            let style = style_after(view, at);
            assert_ne!(style.bold, Some(true));
        });
    }

    /// Moves the caret to the end of the paragraph `down` after its own.
    fn paste_at_end(view: &mut crate::DocumentView, down: usize) -> (usize, usize) {
        let paragraph = view.insert_at.0 + down;
        let end = view.document.paragraph_text(paragraph).unwrap().len();
        view.document.set_selection_range(None);
        view.insert_at = (paragraph, end);
        view.document.place_caret(paragraph, end);
        (paragraph, end)
    }

    /// 記号と特殊文字: a code typed into 文字コード goes in over the
    /// selection and joins the recently used row; 日付と時刻 puts today in as
    /// text, or with 自動的に更新する as a DATE field.
    #[gpui::test]
    fn symbols_and_dates_go_in_at_the_caret(cx: &mut gpui::TestAppContext) {
        use crate::{InsertDateTime, InsertSymbol};
        let (view, cx) = open_selected(cx);
        cx.dispatch_action(InsertSymbol);
        cx.simulate_input("2603");
        cx.simulate_keystrokes("enter");
        let paragraph = view.update(cx, |view, _| {
            assert!(view.characters.dialog.is_none());
            assert!(view.current_paragraph_text().starts_with('☃'));
            assert_eq!(view.characters.recent_symbols, ['☃']);
            view.insert_at.0
        });
        cx.dispatch_action(InsertDateTime);
        let today = view.update(cx, |view, cx| {
            let Some(Dialog::DateTime(dialog)) = view.characters.dialog.as_mut() else {
                panic!("the dialog opens");
            };
            dialog.english = false;
            dialog.chosen = 1;
            dialog.auto_update = true;
            let today =
                jade_office_model::word::insert::format_date(dialog.formats()[1], &dialog.now);
            super::confirm(view, cx);
            today
        });
        view.update(cx, |view, _| {
            let text = view.document.paragraph_text(paragraph).unwrap();
            assert!(text.contains(&today), "{text}");
            let bytes = view.document.export_docx().unwrap();
            let package = jade_opc::OpcPackage::open(&bytes).unwrap();
            let xml = package.part_str("word/document.xml").unwrap().unwrap();
            assert!(xml.contains("DATE \\@ &quot;yyyy年M月d日&quot;"), "{xml}");
        });
    }

    /// ルビ: 読み方 split per character takes よ over 読み and かた over 方,
    /// typed into the dialog; opening it again shows them; undo takes both
    /// off.
    #[gpui::test]
    fn ruby_goes_on_per_character_and_reads_back(cx: &mut gpui::TestAppContext) {
        use crate::Undo;
        let (view, cx) = open_selected(cx);
        let paragraph = view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            view.document
                .insert_text(paragraph, 0, "読み方", 1)
                .unwrap();
            view.document
                .select((paragraph, 0), (paragraph, "読み方".len()), 1)
                .unwrap();
            view.run_ribbon(Action::Characters(CharAction::OpenRuby), cx);
            let Some(Dialog::Ruby(state)) = view.characters.dialog.as_mut() else {
                panic!("the dialog opens");
            };
            let text = state.text.clone();
            state.dialog.split_per_character(&text);
            assert_eq!(state.dialog.groups.len(), 2);
            paragraph
        });
        cx.simulate_input("よ");
        view.update(cx, |view, _| super::ruby::focus_for_test(view, 1));
        cx.simulate_input("かた");
        cx.simulate_keystrokes("enter");
        let xml = view.read_with(cx, |view, _| document_xml(view));
        assert_eq!(xml.matches("<w:ruby>").count(), 2, "{xml}");
        assert!(xml.contains(">かた</w:t>"));
        view.update(cx, |view, cx| {
            view.document
                .select((paragraph, 0), (paragraph, "読み方".len()), 1)
                .unwrap();
            view.run_ribbon(Action::Characters(CharAction::OpenRuby), cx);
            let Some(Dialog::Ruby(state)) = view.characters.dialog.as_ref() else {
                panic!("the dialog opens again");
            };
            let guides: Vec<&str> = state
                .dialog
                .groups
                .iter()
                .map(|group| group.ruby.as_str())
                .collect();
            assert_eq!(guides, ["よ", "かた"]);
            super::cancel(view, cx);
        });
        cx.dispatch_action(Undo);
        let xml = view.read_with(cx, |view, _| document_xml(view));
        assert!(!xml.contains("<w:ruby>"));
    }

    /// 囲い文字 on the selection's first character: □ round it, written
    /// as Word's EQ field.
    #[gpui::test]
    fn an_enclosed_character_replaces_the_selection(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_selected(cx);
        let first = view.update(cx, |view, cx| {
            let first: String = view.current_paragraph_text().chars().take(1).collect();
            let paragraph = view.insert_at.0;
            let end = first.len();
            view.document
                .select((paragraph, 0), (paragraph, end), 1)
                .unwrap();
            view.run_ribbon(Action::Characters(CharAction::OpenEnclose), cx);
            assert_eq!(view.characters.field.text, first);
            let Some(Dialog::Enclose(dialog)) = view.characters.dialog.as_mut() else {
                panic!("the dialog opens");
            };
            dialog.enclosure = 1;
            super::confirm(view, cx);
            first
        });
        let xml = view.read_with(cx, |view, _| document_xml(view));
        assert!(xml.contains(&format!("EQ \\o\\ac(□,{first})")), "{xml}");
    }

    /// 文字種の変換: 全角, then 半角, change the selected text.
    #[gpui::test]
    fn change_case_converts_widths(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_selected(cx);
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            let before = view.current_paragraph_text();
            view.run_ribbon(
                Action::Characters(CharAction::ChangeCase(HeadlessChangeCaseMode::FullWidth)),
                cx,
            );
            let wide = view.document.paragraph_text(paragraph).unwrap();
            view.run_ribbon(
                Action::Characters(CharAction::ChangeCase(HeadlessChangeCaseMode::HalfWidth)),
                cx,
            );
            let narrow = view.document.paragraph_text(paragraph).unwrap();
            let expected = jade_docx_editor_widths(&before);
            assert_eq!(narrow, expected.1, "{wide}");
            assert_eq!(wide, expected.0);
        });
    }

    /// What 全角 and 半角 make of the paragraph's first three characters.
    fn jade_docx_editor_widths(text: &str) -> (String, String) {
        let end = text.char_indices().nth(3).map(|(index, _)| index).unwrap();
        let (head, tail) = text.split_at(end);
        let wide: String = head
            .chars()
            .map(|ch| match ch {
                ' ' => '\u{3000}',
                '!'..='~' if ch != '\\' => char::from_u32(ch as u32 + 0xFEE0).unwrap(),
                _ => ch,
            })
            .collect();
        let narrow: String = wide
            .chars()
            .map(|ch| match ch {
                '\u{3000}' => ' ',
                '\u{FF01}'..='\u{FF5E}' => char::from_u32(ch as u32 - 0xFEE0).unwrap(),
                _ => ch,
            })
            .collect();
        (format!("{wide}{tail}"), format!("{narrow}{tail}"))
    }
}

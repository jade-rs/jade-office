//! 挿入's 記号と特殊文字 and 日付と時刻 dialogs.
//!
//! 記号と特殊文字: the 記号 tab's grid of a Unicode block (種類), the
//! recently used row and a 文字コード to type; the 特殊文字 tab's dashes,
//! spaces and marks. 日付と時刻: Word's formats for 日本語 (グレゴリオ暦 or
//! 和暦) and 英語, shown with today's date, and 自動的に更新する, which
//! inserts a DATE or TIME field instead of text.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_web::word::insert::{
    character_from_code, format_date, FieldDateTime, SymbolSubset, WordInsert,
    ENGLISH_DATE_FORMATS, JAPANESE_DATE_FORMATS, JAPANESE_ERA_FORMATS, SPECIAL_CHARACTERS,
    SYMBOL_SUBSETS,
};

use super::{caption, check, list_row, Dialog};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

/// The 記号と特殊文字 dialog.
pub struct SymbolDialog {
    /// The 特殊文字 tab rather than 記号.
    pub special: bool,
    pub subset: usize,
    pub chosen: Option<char>,
}

/// The 日付と時刻 dialog.
pub struct DateTimeDialog {
    pub english: bool,
    pub era: bool,
    pub chosen: usize,
    pub auto_update: bool,
    pub now: FieldDateTime,
}

impl DateTimeDialog {
    pub fn formats(&self) -> &'static [&'static str] {
        match (self.english, self.era) {
            (true, _) => &ENGLISH_DATE_FORMATS,
            (false, true) => &JAPANESE_ERA_FORMATS,
            (false, false) => &JAPANESE_DATE_FORMATS,
        }
    }
}

/// The local date and time, as fields show it: the browser's clock.
pub fn local_now() -> FieldDateTime {
    let now = js_sys::Date::new_0();
    FieldDateTime {
        year: now.get_full_year() as i32,
        month: now.get_month() + 1,
        day: now.get_date(),
        hour: now.get_hours(),
        minute: now.get_minutes(),
        second: now.get_seconds(),
        weekday: now.get_day(),
    }
}

pub fn open_symbols(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let subset = view
        .characters
        .recent_symbols
        .first()
        .and_then(|ch| SymbolSubset::of(*ch))
        .unwrap_or(0);
    view.characters.open(Dialog::Symbol(SymbolDialog {
        special: false,
        subset,
        chosen: None,
    }));
    cx.notify();
}

pub fn open_date_time(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    view.characters.open(Dialog::DateTime(DateTimeDialog {
        english: i18n::is_english(),
        era: false,
        chosen: 0,
        auto_update: false,
        now: local_now(),
    }));
    cx.notify();
}

fn after_insert(
    view: &mut DocumentView,
    done: Result<(usize, usize), String>,
    cx: &mut Context<DocumentView>,
) {
    let done = done.map(|caret| {
        if let Some(document) = view.word_mut() {
            document.place_caret(caret.0, caret.1);
        }
    });
    view.after_character_edit(done, cx);
}

/// 挿入: the chosen symbol, or the one the typed code names.
pub(super) fn confirm_symbol(
    view: &mut DocumentView,
    dialog: SymbolDialog,
    cx: &mut Context<DocumentView>,
) {
    let Some(symbol) = dialog.chosen else {
        return cx.notify();
    };
    view.characters.recent_symbols.retain(|ch| *ch != symbol);
    view.characters.recent_symbols.insert(0, symbol);
    view.characters.recent_symbols.truncate(16);
    let done = view
        .word_mut()
        .map(|document| WordInsert::insert_symbol(document, symbol))
        .unwrap_or_else(|| Err("no Word document".to_string()));
    after_insert(view, done, cx);
}

/// OK: today in the chosen format, as text or as a field.
pub(super) fn confirm_date_time(
    view: &mut DocumentView,
    dialog: DateTimeDialog,
    cx: &mut Context<DocumentView>,
) {
    let Some(picture) = dialog.formats().get(dialog.chosen) else {
        return;
    };
    let done = view
        .word_mut()
        .map(|document| {
            WordInsert::insert_date_time(document, picture, &dialog.now, dialog.auto_update)
        })
        .unwrap_or_else(|| Err("no Word document".to_string()));
    after_insert(view, done, cx);
}

fn tab(
    id: &'static str,
    label: &'static str,
    on: bool,
    special: bool,
    cx: &mut Context<DocumentView>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_0p5()
        .rounded_md()
        .cursor_pointer()
        .when(on, |chip| chip.bg(theme::selected()))
        .hover(|chip| chip.bg(theme::hovered()))
        .child(i18n::text(label))
        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
            if let Some(Dialog::Symbol(dialog)) = view.characters.dialog.as_mut() {
                dialog.special = special;
                dialog.chosen = None;
            }
            cx.notify();
        }))
}

fn symbol_cell(
    id: impl Into<gpui::ElementId>,
    symbol: char,
    on: bool,
    cx: &mut Context<DocumentView>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when(on, |cell| cell.bg(theme::selected()))
        .text_size(px(16.0))
        .cursor_pointer()
        .hover(|cell| cell.bg(theme::hovered()))
        .child(symbol.to_string())
        .on_click(cx.listener(move |view, event: &ClickEvent, _, cx| {
            if let Some(Dialog::Symbol(dialog)) = view.characters.dialog.as_mut() {
                dialog.chosen = Some(symbol);
            }
            view.characters.field.text.clear();
            view.characters.field.caret = 0;
            // A double click inserts, as in Word.
            if event.click_count() >= 2 {
                super::confirm(view, cx);
            }
            cx.notify();
        }))
}

pub(super) fn symbol_body(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Symbol(dialog)) = view.characters.dialog.as_ref() else {
        return div().into_any_element();
    };
    let (special, subset, chosen) = (dialog.special, dialog.subset, dialog.chosen);
    let typed = view.characters.field.text.clone();
    let typed_char = character_from_code(&typed);
    let current = typed_char.or(chosen);
    let recent = view.characters.recent_symbols.clone();
    let tabs = div()
        .flex()
        .gap_1()
        .child(tab(
            "symbol-tab",
            "chars.symbol.tab_symbols",
            !special,
            false,
            cx,
        ))
        .child(tab(
            "special-tab",
            "chars.symbol.tab_special",
            special,
            true,
            cx,
        ));
    let content = if special {
        div()
            .flex()
            .flex_col()
            .max_h(px(320.0))
            .children(
                SPECIAL_CHARACTERS
                    .iter()
                    .enumerate()
                    .map(|(index, (symbol, key))| {
                        let symbol = *symbol;
                        list_row(
                            ("special", index),
                            format!("{}    {}", i18n::text(key), shown(symbol)),
                            chosen == Some(symbol),
                        )
                        .on_click(cx.listener(
                            move |view, _: &ClickEvent, _, cx| {
                                if let Some(Dialog::Symbol(dialog)) =
                                    view.characters.dialog.as_mut()
                                {
                                    dialog.chosen = Some(symbol);
                                }
                                cx.notify();
                            },
                        ))
                    }),
            )
            .into_any_element()
    } else {
        let subsets =
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(SYMBOL_SUBSETS.iter().enumerate().map(|(index, entry)| {
                    list_row(("subset", index), i18n::text(entry.key), index == subset).on_click(
                        cx.listener(move |view, _: &ClickEvent, _, cx| {
                            if let Some(Dialog::Symbol(dialog)) = view.characters.dialog.as_mut() {
                                dialog.subset = index;
                            }
                            cx.notify();
                        }),
                    )
                }));
        let characters = SYMBOL_SUBSETS[subset].characters();
        let mut grid = div()
            .id("symbol-grid")
            .flex()
            .flex_col()
            .max_h(px(240.0))
            .overflow_y_scroll();
        for (row, chunk) in characters.chunks(16).enumerate() {
            grid = grid.child(div().flex().children(chunk.iter().enumerate().map(
                |(column, symbol)| {
                    symbol_cell(
                        ("symbol", row * 16 + column),
                        *symbol,
                        current == Some(*symbol),
                        cx,
                    )
                },
            )));
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(caption(i18n::text("chars.symbol.subset")))
            .child(subsets)
            .child(grid)
            .child(caption(i18n::text("chars.symbol.recent")))
            .child(
                div()
                    .flex()
                    .children(recent.iter().enumerate().map(|(index, symbol)| {
                        symbol_cell(("recent", index), *symbol, current == Some(*symbol), cx)
                    })),
            )
            .into_any_element()
    };
    let code = current
        .map(|symbol| format!("U+{:04X}", symbol as u32))
        .unwrap_or_default();
    div()
        .w(px(520.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(tabs)
        .child(content)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(caption(i18n::text("chars.symbol.code")))
                .child(
                    div()
                        .min_w(px(90.0))
                        .px_1()
                        .border_1()
                        .border_color(theme::accent())
                        .bg(theme::canvas())
                        .child(if typed.is_empty() {
                            code
                        } else {
                            format!("{typed}▏")
                        }),
                )
                .child(
                    div()
                        .text_size(px(28.0))
                        .child(current.map(shown).unwrap_or_default()),
                ),
        )
        .into_any_element()
}

/// A character as the dialog shows it: spaces and invisible marks named by
/// their code, so the row is not blank.
fn shown(symbol: char) -> String {
    if symbol.is_whitespace() || matches!(symbol, '\u{00AD}' | '\u{200B}' | '\u{200D}') {
        format!("U+{:04X}", symbol as u32)
    } else {
        symbol.to_string()
    }
}

/// A 言語 or カレンダー choice's listener: changes the dialog and goes
/// back to its first format.
fn date_setter(
    change: fn(&mut DateTimeDialog),
    cx: &mut Context<DocumentView>,
) -> impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static {
    // `cx.listener`, without its borrow of `cx` in the returned type.
    let view = cx.weak_entity();
    move |_, _, cx| {
        view.update(cx, |view, cx| {
            if let Some(Dialog::DateTime(dialog)) = view.characters.dialog.as_mut() {
                change(dialog);
                dialog.chosen = 0;
            }
            cx.notify();
        })
        .ok();
    }
}

pub(super) fn date_time_body(
    view: &mut DocumentView,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let Some(Dialog::DateTime(dialog)) = view.characters.dialog.as_ref() else {
        return div().into_any_element();
    };
    let (english, era, chosen, auto_update) = (
        dialog.english,
        dialog.era,
        dialog.chosen,
        dialog.auto_update,
    );
    let rows: Vec<_> = dialog
        .formats()
        .iter()
        .enumerate()
        .map(|(index, picture)| {
            list_row(
                ("date-format", index),
                format_date(picture, &dialog.now),
                index == chosen,
            )
            .on_click(cx.listener(move |view, event: &ClickEvent, _, cx| {
                if let Some(Dialog::DateTime(dialog)) = view.characters.dialog.as_mut() {
                    dialog.chosen = index;
                }
                if event.click_count() >= 2 {
                    super::confirm(view, cx);
                }
                cx.notify();
            }))
        })
        .collect();
    let set = date_setter;
    div()
        .w(px(360.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(caption(i18n::text("chars.date.language")))
        .child(
            div()
                .flex()
                .gap_3()
                .child(
                    check("date-ja", i18n::text("chars.date.japanese"), !english)
                        .on_click(set(|dialog| dialog.english = false, cx)),
                )
                .child(
                    check("date-en", i18n::text("chars.date.english"), english)
                        .on_click(set(|dialog| dialog.english = true, cx)),
                ),
        )
        .when(!english, |body| {
            body.child(caption(i18n::text("chars.date.calendar")))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            check("date-gregorian", i18n::text("chars.date.gregorian"), !era)
                                .on_click(set(|dialog| dialog.era = false, cx)),
                        )
                        .child(
                            check("date-era", i18n::text("chars.date.era"), era)
                                .on_click(set(|dialog| dialog.era = true, cx)),
                        ),
                )
        })
        .child(caption(i18n::text("chars.date.formats")))
        .child(
            div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(theme::border())
                .p_1()
                .children(rows),
        )
        .child(
            check(
                "date-auto",
                i18n::text("chars.date.auto_update"),
                auto_update,
            )
            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                if let Some(Dialog::DateTime(dialog)) = view.characters.dialog.as_mut() {
                    dialog.auto_update = !auto_update;
                }
                cx.notify();
            })),
        )
        .into_any_element()
}
